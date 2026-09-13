//! Root-private capability broker: C -> F + one capture socket, or E.
use clap::Parser;
use dwdesktop_console::{input::peer_root, validate_root_socket};
use std::{
    fs,
    io::{self, Read},
    mem,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const MAX_PEERS: usize = 8;
static STOP: AtomicBool = AtomicBool::new(false);

#[derive(Parser)]
struct Args {
    #[arg(long)]
    socket: PathBuf,
    #[arg(long)]
    capture_socket: PathBuf,
}

fn rejected() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "capture broker request rejected",
    )
}

fn capture_connect(path: &Path) -> io::Result<UnixStream> {
    validate_root_socket(path)?;
    let bytes = path.as_os_str().as_bytes();
    let mut address: libc::sockaddr_un = unsafe { mem::zeroed() };
    if bytes.is_empty() || bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
        return Err(rejected());
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (destination, source) in address.sun_path.iter_mut().zip(bytes) {
        *destination = *source as libc::c_char;
    }
    // AF_UNIX nonblocking connect fails immediately when its accept queue is full.
    // Never let an unavailable capture daemon pin a broker worker indefinitely.
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let length = mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;
    if unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            length as libc::socklen_t,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(false)?;
    peer_root(&stream)?;
    Ok(stream)
}

fn send_result(stream: &UnixStream, capture: Option<RawFd>) -> io::Result<()> {
    let mut byte = if capture.is_some() { b'F' } else { b'E' };
    let mut vector = libc::iovec {
        iov_base: (&mut byte as *mut u8).cast(),
        iov_len: 1,
    };
    // cmsghdr-typed storage provides the required native control-message alignment.
    let mut control: [libc::cmsghdr; 2] = unsafe { mem::zeroed() };
    let mut message: libc::msghdr = unsafe { mem::zeroed() };
    message.msg_iov = &mut vector;
    message.msg_iovlen = 1;
    if let Some(fd) = capture {
        let space = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as u32) } as usize;
        if space > mem::size_of_val(&control) {
            return Err(rejected());
        }
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = space;
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&message);
            if header.is_null() {
                return Err(rejected());
            }
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as u32) as usize;
            libc::CMSG_DATA(header).cast::<RawFd>().write_unaligned(fd);
        }
    }
    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
        stream.set_write_timeout(Some(remaining))?;
        let sent = unsafe { libc::sendmsg(stream.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
        // A successful rights transfer must never be retried, even after deadline.
        if sent == 1 {
            return Ok(());
        }
        if sent >= 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn command(
    stream: &UnixStream,
    byte: u8,
    connect: impl FnOnce() -> io::Result<UnixStream>,
) -> io::Result<()> {
    if byte != b'C' {
        return Err(rejected());
    }
    match connect() {
        Ok(capture) => send_result(stream, Some(capture.as_raw_fd())),
        Err(_) => send_result(stream, None),
    } // The broker's connected descriptor closes immediately after sendmsg.
}

fn serve(mut stream: UnixStream, capture_path: &Path) -> io::Result<()> {
    peer_root(&stream)?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    while !STOP.load(Ordering::Relaxed) {
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => command(&stream, byte[0], || capture_connect(capture_path))?,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                ) =>
            {
                ()
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

struct SocketPath {
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl SocketPath {
    fn new(path: PathBuf) -> io::Result<Self> {
        let meta = fs::symlink_metadata(&path)?;
        if !meta.file_type().is_socket() {
            return Err(rejected());
        }
        Ok(Self {
            path,
            device: meta.dev(),
            inode: meta.ino(),
        })
    }
}
impl Drop for SocketPath {
    fn drop(&mut self) {
        if let Ok(meta) = fs::symlink_metadata(&self.path) {
            if meta.file_type().is_socket() && meta.dev() == self.device && meta.ino() == self.inode
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

extern "C" fn stop_signal(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}
fn install_signals() -> io::Result<()> {
    let mut action: libc::sigaction = unsafe { mem::zeroed() };
    action.sa_sigaction = stop_signal as *const () as usize;
    unsafe {
        libc::sigemptyset(&mut action.sa_mask);
    }
    for signal in [libc::SIGTERM, libc::SIGINT] {
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    if !args.socket.is_absolute()
        || !args.capture_socket.is_absolute()
        || args.socket == args.capture_socket
    {
        return Err(rejected());
    }
    let parent = args.socket.parent().ok_or_else(rejected)?;
    let meta = fs::symlink_metadata(parent)?;
    if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o077 != 0 {
        return Err(rejected());
    }
    install_signals()?;
    unsafe {
        libc::umask(0o077);
    }
    // Refuse existing paths. Capture may legitimately be unavailable at startup.
    let listener = UnixListener::bind(&args.socket)?;
    let _socket_path = SocketPath::new(args.socket)?;
    fs::set_permissions(&_socket_path.path, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let capture_path = Arc::new(args.capture_socket);
    let mut workers: Vec<std::thread::JoinHandle<()>> = Vec::new();
    let result = (|| -> io::Result<()> {
        while !STOP.load(Ordering::Relaxed) {
            let mut index = 0;
            while index < workers.len() {
                if workers[index].is_finished() {
                    if workers.swap_remove(index).join().is_err() {
                        return Err(io::Error::other("capture broker worker failed"));
                    }
                } else {
                    index += 1;
                }
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    if workers.len() >= MAX_PEERS {
                        drop(stream);
                        continue;
                    }
                    stream.set_nonblocking(false)?;
                    let path = Arc::clone(&capture_path);
                    workers.push(
                        std::thread::Builder::new()
                            .name("capture-fd-peer".into())
                            .spawn(move || {
                                let _ = serve(stream, &path);
                            })?,
                    );
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let mut fd = libc::pollfd {
                        fd: listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    if unsafe { libc::poll(&mut fd, 1, 100) } < 0
                        && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    })();
    STOP.store(true, Ordering::Relaxed);
    for worker in workers {
        let _ = worker.join();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn receive(stream: &UnixStream) -> (u8, Option<OwnedFd>) {
        let mut byte = 0u8;
        let mut vector = libc::iovec {
            iov_base: (&mut byte as *mut u8).cast(),
            iov_len: 1,
        };
        let mut control: [libc::cmsghdr; 2] = unsafe { mem::zeroed() };
        let mut message: libc::msghdr = unsafe { mem::zeroed() };
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = mem::size_of_val(&control);
        let count =
            unsafe { libc::recvmsg(stream.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
        assert_eq!(count, 1);
        assert_eq!(message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC), 0);
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&message);
            if header.is_null() {
                return (byte, None);
            }
            assert_eq!((*header).cmsg_level, libc::SOL_SOCKET);
            assert_eq!((*header).cmsg_type, libc::SCM_RIGHTS);
            assert_eq!(
                (*header).cmsg_len,
                libc::CMSG_LEN(mem::size_of::<RawFd>() as u32) as usize
            );
            assert!(libc::CMSG_NXTHDR(&message, header).is_null());
            (
                byte,
                Some(OwnedFd::from_raw_fd(
                    libc::CMSG_DATA(header).cast::<RawFd>().read_unaligned(),
                )),
            )
        }
    }
    #[test]
    fn transfers_one_live_capture_fd_and_closes_sender_copy() {
        let (broker, receiver) = UnixStream::pair().unwrap();
        let (capture, mut source) = UnixStream::pair().unwrap();
        command(&broker, b'C', || Ok(capture)).unwrap();
        let (byte, fd) = receive(&receiver);
        assert_eq!(byte, b'F');
        let mut capture = UnixStream::from(fd.unwrap());
        source.write_all(b"frame").unwrap();
        let mut frame = [0; 5];
        capture.read_exact(&mut frame).unwrap();
        assert_eq!(&frame, b"frame");
        drop(capture);
        assert_eq!(source.read(&mut frame).unwrap(), 0);
    }
    #[test]
    fn unavailable_then_retry_on_same_connection() {
        let (broker, receiver) = UnixStream::pair().unwrap();
        command(&broker, b'C', || Err(io::ErrorKind::NotFound.into())).unwrap();
        let (byte, fd) = receive(&receiver);
        assert_eq!(byte, b'E');
        assert!(fd.is_none());
        let (capture, _source) = UnixStream::pair().unwrap();
        command(&broker, b'C', || Ok(capture)).unwrap();
        let (byte, fd) = receive(&receiver);
        assert_eq!(byte, b'F');
        assert!(fd.is_some());
    }
    #[test]
    fn wrong_command_never_connects() {
        let (broker, _receiver) = UnixStream::pair().unwrap();
        assert!(
            command(&broker, b'/', || panic!(
                "invalid command attempted connect"
            ))
            .is_err()
        );
    }
    #[test]
    fn blocked_receiver_has_bounded_send() {
        let (mut broker, _receiver) = UnixStream::pair().unwrap();
        broker.set_nonblocking(true).unwrap();
        loop {
            match broker.write(&[0]) {
                Ok(1) => (),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                other => panic!("unexpected fill: {other:?}"),
            }
        }
        broker.set_nonblocking(false).unwrap();
        let start = Instant::now();
        assert!(send_result(&broker, None).is_err());
        assert!(start.elapsed() < Duration::from_millis(250));
    }
}
