use clap::Parser;
use dwdesktop_console::{input::*, input_device::InputDevice};
use std::{
    fs,
    io::{self, Read, Write},
    os::fd::AsRawFd,
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    socket: PathBuf,
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    #[arg(long)]
    guard_state: Option<PathBuf>,
}

static STOP: AtomicBool = AtomicBool::new(false);
const MAX_PEERS: usize = 8;

struct SharedDevice<D> {
    device: D,
    owner: Option<u64>,
}
impl<D> SharedDevice<D> {
    fn claim(&mut self, peer: u64) -> bool {
        if self.owner.is_some() {
            return false;
        }
        self.owner = Some(peer);
        true
    }
    fn owned(&mut self, peer: u64) -> io::Result<&mut D> {
        if self.owner != Some(peer) {
            return Err(invalid());
        }
        Ok(&mut self.device)
    }
    fn release(
        &mut self,
        peer: u64,
        reset: impl FnOnce(&mut D) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.owner != Some(peer) {
            return Ok(());
        }
        reset(&mut self.device)?;
        self.owner = None;
        Ok(())
    }
}
type SharedInput = Arc<Mutex<SharedDevice<InputDevice>>>;
fn locked(shared: &SharedInput) -> io::Result<MutexGuard<'_, SharedDevice<InputDevice>>> {
    shared.lock().map_err(|_| {
        STOP.store(true, Ordering::Relaxed);
        invalid()
    })
}
fn release_owned(shared: &SharedInput, peer: u64) -> io::Result<()> {
    let result = locked(shared)?.release(peer, InputDevice::reset);
    if result.is_err() {
        STOP.store(true, Ordering::Relaxed);
    }
    result
}
extern "C" fn stop_signal(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

fn install_signals() -> io::Result<()> {
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Guard {
    session: String,
    width: u32,
    height: u32,
    valid: bool,
}

fn read_guard(path: &Path, width: u32, height: u32) -> io::Result<Guard> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(invalid());
    }
    // Protect resolution of the filename too; only root may replace ancestors.
    for parent in path.ancestors().skip(1) {
        let meta = fs::symlink_metadata(parent)?;
        if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err(invalid());
        }
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file()
        || meta.uid() != 0
        || meta.mode() & 0o022 != 0
        || meta.len() > MAX_INPUT as u64
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT {
        return Err(invalid());
    }
    let guard: Guard = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if SystemTime::now()
        .duration_since(meta.modified()?)
        .map_err(|_| invalid())?
        > Duration::from_millis(500)
        || !guard.valid
        || guard.session.is_empty()
        || guard.session.len() > 256
        || guard.width != width
        || guard.height != height
    {
        return Err(invalid());
    }
    Ok(guard)
}

struct Control {
    generation: String,
    sequence: u64,
    deadline: Instant,
    end: Option<Instant>,
    guard: Option<Guard>,
}
impl Control {
    fn valid(&self, path: Option<&Path>, width: u32, height: u32) -> bool {
        if Instant::now() >= self.deadline {
            return false;
        }
        match (path, &self.guard) {
            (Some(path), Some(baseline)) => {
                read_guard(path, width, height).is_ok_and(|current| current == *baseline)
            }
            (None, None) => true,
            _ => false,
        }
    }
}

#[derive(Default)]
struct FrameReader {
    bytes: Vec<u8>,
    total: Option<usize>,
    deadline: Option<Instant>,
}
impl FrameReader {
    // One bounded read per poll; offsets survive timeout/guard invalidation.
    fn poll(&mut self, stream: &mut UnixStream, wake: Instant) -> io::Result<Option<Vec<u8>>> {
        if self.deadline.is_some_and(|end| Instant::now() >= end) {
            return Err(invalid());
        }
        let until = self.deadline.map_or(wake, |end| wake.min(end));
        let timeout = match remaining(until) {
            Ok(timeout) => timeout,
            Err(_) => return Ok(None),
        };
        stream.set_read_timeout(Some(timeout))?;
        let target = self.total.unwrap_or(4);
        let mut buffer = [0u8; MAX_INPUT + 4];
        let n = match stream.read(&mut buffer[..target - self.bytes.len()]) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                ) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        if self.deadline.is_none() {
            self.deadline = Some(Instant::now() + Duration::from_millis(LEASE_MS));
        }
        self.bytes.extend_from_slice(&buffer[..n]);
        if self.deadline.is_some_and(|end| Instant::now() >= end) {
            return Err(invalid());
        }
        if self.total.is_none() && self.bytes.len() == 4 {
            let length =
                u32::from_be_bytes(self.bytes[..4].try_into().map_err(|_| invalid())?) as usize;
            if length == 0 || length > MAX_INPUT {
                return Err(invalid());
            }
            self.total = Some(length + 4);
        }
        if self.total == Some(self.bytes.len()) {
            let frame = self.bytes.split_off(4);
            self.bytes.clear();
            self.total = None;
            self.deadline = None;
            return Ok(Some(frame));
        }
        Ok(None)
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "input lease expired"))
}

fn send(
    stream: &mut UnixStream,
    message: &impl serde::Serialize,
    deadline: Instant,
) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    if body.is_empty() || body.len() > MAX_INPUT {
        return Err(invalid());
    }
    let mut packet = Vec::with_capacity(body.len() + 4);
    packet.extend_from_slice(&(body.len() as u32).to_be_bytes());
    packet.extend_from_slice(&body);
    // One budget for the entire response, including partial writes/EINTR.
    let deadline = deadline.min(Instant::now() + Duration::from_millis(100));
    let mut bytes = packet.as_slice();
    while !bytes.is_empty() {
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        match stream.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }
    remaining(deadline)?;
    Ok(())
}

fn next_deadline(
    event: &Event,
    accepted_at: Instant,
    deadline: Instant,
    end: Instant,
) -> io::Result<Instant> {
    if accepted_at >= deadline || accepted_at >= end {
        return Err(invalid());
    }
    Ok(if matches!(event, Event::Renew {}) {
        (accepted_at + Duration::from_millis(LEASE_MS)).min(end)
    } else {
        deadline
    })
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
            return Err(invalid());
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
        // Directory is root-private. Never remove a replacement file/socket.
        if let Ok(meta) = fs::symlink_metadata(&self.path) {
            if meta.file_type().is_socket() && meta.dev() == self.device && meta.ino() == self.inode
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

fn serve(
    stream: &mut UnixStream,
    shared: &SharedInput,
    peer: u64,
    width: u32,
    height: u32,
    guard_path: Option<&Path>,
) -> io::Result<()> {
    peer_root(stream)?;
    let mut control: Option<Control> = None;
    let mut reader = FrameReader::default();
    loop {
        if STOP.load(Ordering::Relaxed) {
            release_owned(shared, peer)?;
            return Ok(());
        }
        if control
            .as_ref()
            .is_some_and(|c| !c.valid(guard_path, width, height))
        {
            release_owned(shared, peer)?;
            control = None;
            unavailable(stream, None)?;
        }
        let wake = Instant::now() + Duration::from_millis(100);
        let wake = control.as_ref().map_or(wake, |c| wake.min(c.deadline));
        let Some(bytes) = reader.poll(stream, wake)? else {
            continue;
        };
        if STOP.load(Ordering::Relaxed) {
            release_owned(shared, peer)?;
            return Ok(());
        }
        // A read may straddle expiry or a session transition. Never inject first.
        if control
            .as_ref()
            .is_some_and(|c| !c.valid(guard_path, width, height))
        {
            release_owned(shared, peer)?;
            control = None;
            unavailable(stream, None)?;
        }
        if control.is_none() {
            let request_id = match serde_json::from_slice::<Acquire>(&bytes) {
                Ok(Acquire::Acquire { request_id }) => {
                    if request_id
                        .as_ref()
                        .is_some_and(|id| id.is_empty() || id.len() > 64)
                    {
                        return Err(invalid());
                    }
                    request_id
                }
                Err(_) => {
                    // Preserve framing for queued old-generation messages, but do not execute them.
                    serde_json::from_slice::<Request>(&bytes).map_err(|_| invalid())?;
                    unavailable(stream, None)?;
                    continue;
                }
            };
            let guard = match guard_path {
                Some(path) => match read_guard(path, width, height) {
                    Ok(guard) => Some(guard),
                    Err(_) => {
                        unavailable(stream, request_id)?;
                        continue;
                    }
                },
                None => None,
            };
            let mut entropy = [0u8; 16];
            fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
            let generation: String = entropy.iter().map(|b| format!("{b:02x}")).collect();
            let now = Instant::now();
            let state = Control {
                generation: generation.clone(),
                sequence: 1,
                deadline: now + Duration::from_millis(LEASE_MS),
                end: guard.is_none().then_some(now + Duration::from_secs(10)),
                guard,
            };
            // No socket I/O while holding the global input owner lock.
            let claimed = { locked(shared)?.claim(peer) };
            if !claimed {
                unavailable(stream, request_id)?;
                continue;
            }
            send(
                stream,
                &Hello {
                    kind: "ready".into(),
                    protocol: "dwconsole.input".into(),
                    version: "0.2.0".into(),
                    generation,
                    width,
                    height,
                    lease_ms: LEASE_MS,
                },
                state.deadline,
            )?;
            control = Some(state);
            continue;
        }
        let state = control.as_mut().ok_or_else(invalid)?;
        let request: Request = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        validate(&request, &state.generation, state.sequence, width, height)?;
        let accepted = Instant::now();
        let end = state
            .end
            .unwrap_or(accepted + Duration::from_millis(LEASE_MS));
        state.deadline = next_deadline(&request.event, accepted, state.deadline, end)?;
        let release = matches!(request.event, Event::Release {});
        {
            let mut shared = locked(shared)?;
            let device = shared.owned(peer)?;
            // Owner check and the complete event execute atomically against takeover.
            match request.event {
                Event::Move { x, y } => device.move_to(x, y)?,
                Event::Button { button, down, x, y } => {
                    device.move_to(x, y)?;
                    device.button(button, down)?;
                }
                Event::Wheel { vertical, horizontal, x, y } => {
                    device.move_to(x, y)?;
                    device.wheel(vertical, horizontal)?;
                }
                Event::Key { hid, down } => device.key(hid, down)?,
                Event::Reset {} | Event::Release {} => device.reset()?,
                Event::Renew {} => (),
            }
            if release {
                shared.owner = None;
            }
        }
        send(
            stream,
            &Ack {
                kind: "ack".into(),
                sequence: state.sequence,
                accepted: true,
            },
            state.deadline,
        )?;
        state.sequence = state.sequence.checked_add(1).ok_or_else(invalid)?;
        if release {
            control = None;
        }
    }
}

fn unavailable(stream: &mut UnixStream, request_id: Option<String>) -> io::Result<()> {
    send(
        stream,
        &Unavailable {
            kind: "unavailable".into(),
            reason: "control expired".into(),
            request_id,
        },
        Instant::now() + Duration::from_millis(100),
    )
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    install_signals()?;
    if !(2..=4096).contains(&args.width)
        || !(2..=4096).contains(&args.height)
        || !args.socket.is_absolute()
    {
        return Err(invalid());
    }
    let parent = args.socket.parent().ok_or_else(invalid)?;
    let meta = fs::symlink_metadata(parent)?;
    if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o077 != 0 {
        return Err(invalid());
    }
    // Refuse an existing path; do not unlink somebody else's socket.
    unsafe { libc::umask(0o077) };
    let listener = UnixListener::bind(&args.socket)?;
    listener.set_nonblocking(true)?;
    let _socket_path = SocketPath::new(args.socket.clone())?;
    fs::set_permissions(&args.socket, fs::Permissions::from_mode(0o600))?;
    let device = InputDevice::new(args.width, args.height)?;
    for path in device.device_paths() {
        eprintln!("input device: {path}");
    }
    eprintln!("input probe devices created; compositor adoption requires verification");
    let shared = Arc::new(Mutex::new(SharedDevice {
        device,
        owner: None,
    }));
    let mut workers: Vec<std::thread::JoinHandle<()>> = Vec::new();
    let mut peer = 0u64;
    let result = (|| -> io::Result<()> {
        while !STOP.load(Ordering::Relaxed) {
            let mut index = 0;
            while index < workers.len() {
                if workers[index].is_finished() {
                    if workers.swap_remove(index).join().is_err() {
                        return Err(io::Error::other("input peer worker failed"));
                    }
                } else {
                    index += 1;
                }
            }
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let mut fd = libc::pollfd {
                        fd: listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let result = unsafe { libc::poll(&mut fd, 1, 100) };
                    if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
                    {
                        return Err(io::Error::last_os_error());
                    }
                    continue;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            if workers.len() >= MAX_PEERS {
                drop(stream);
                continue;
            }
            stream.set_nonblocking(false)?;
            peer = peer.checked_add(1).ok_or_else(invalid)?;
            let shared = Arc::clone(&shared);
            let guard_path = args.guard_state.clone();
            let (width, height) = (args.width, args.height);
            workers.push(
                std::thread::Builder::new()
                    .name("console-input-peer".into())
                    .spawn(move || {
                        let result = serve(
                            &mut stream,
                            &shared,
                            peer,
                            width,
                            height,
                            guard_path.as_deref(),
                        );
                        // A nonowner closing (including an idle bridge) must never reset another peer.
                        let reset = release_owned(&shared, peer);
                        eprintln!(
                            "input peer ended; result={}",
                            if result.is_ok() && reset.is_ok() {
                                "closed"
                            } else {
                                "closed-or-rejected"
                            }
                        );
                    })?,
            );
        }
        Ok(())
    })();
    STOP.store(true, Ordering::Relaxed);
    // Every peer polls at <=100 ms and bounds response writes. No unbounded socket wait.
    for worker in workers {
        let _ = worker.join();
    }
    let mut input = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    input.device.reset()?;
    input.owner = None;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simultaneous_acquire_has_exactly_one_owner() {
        let shared = Arc::new(Mutex::new(SharedDevice {
            device: 0usize,
            owner: None,
        }));
        let barrier = Arc::new(std::sync::Barrier::new(MAX_PEERS));
        let mut workers = Vec::new();
        for peer in 1..=MAX_PEERS as u64 {
            let shared = Arc::clone(&shared);
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                shared.lock().unwrap().claim(peer)
            }));
        }
        let wins = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|won| *won)
            .count();
        assert_eq!(wins, 1);
        let mut state = shared.lock().unwrap();
        let owner = state.owner.unwrap();
        for peer in 1..=MAX_PEERS as u64 {
            assert_eq!(state.owned(peer).is_ok(), peer == owner);
        }
    }
    #[test]
    fn nonowner_close_and_delayed_cleanup_cannot_reset_new_owner() {
        let mut shared = SharedDevice {
            device: 0usize,
            owner: None,
        };
        assert!(shared.claim(1));
        assert!(!shared.claim(2));
        shared.release(2, |_| panic!("nonowner reset")).unwrap();
        assert_eq!(shared.owner, Some(1));
        shared
            .release(1, |count| {
                *count += 1;
                Ok(())
            })
            .unwrap();
        assert!(shared.claim(2));
        shared
            .release(1, |_| panic!("old owner reset new controller"))
            .unwrap();
        assert_eq!(shared.owner, Some(2));
        assert_eq!(shared.device, 1);
        assert!(shared.owned(1).is_err());
        *shared.owned(2).unwrap() += 1;
        assert_eq!(shared.device, 2);
    }
    #[test]
    fn failed_reset_does_not_allow_takeover() {
        let mut shared = SharedDevice {
            device: (),
            owner: None,
        };
        assert!(shared.claim(1));
        assert!(
            shared
                .release(1, |_| Err(io::ErrorKind::WouldBlock.into()))
                .is_err()
        );
        assert_eq!(shared.owner, Some(1));
        assert!(!shared.claim(2));
    }
    #[test]
    fn unavailable_correlates_denial_but_not_async_expiry() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        unavailable(&mut a, Some("attempt1".into())).unwrap();
        let message: serde_json::Value =
            serde_json::from_slice(&dwdesktop_console::read_record(&mut b, MAX_INPUT).unwrap())
                .unwrap();
        assert_eq!(message["requestId"], "attempt1");
        unavailable(&mut a, None).unwrap();
        let message: serde_json::Value =
            serde_json::from_slice(&dwdesktop_console::read_record(&mut b, MAX_INPUT).unwrap())
                .unwrap();
        assert!(message.get("requestId").is_none());
    }
    #[test]
    fn only_renew_extends_lease_and_never_past_probe_end() {
        let start = Instant::now();
        let deadline = start + Duration::from_millis(1000);
        let accepted = start + Duration::from_millis(800);
        let end = start + Duration::from_millis(1500);
        for event in [
            Event::Move { x: 0, y: 0 },
            Event::Key {
                hid: 41,
                down: true,
            },
            Event::Button {
                button: 1,
                down: true,
                x: 0,
                y: 0,
            },
            Event::Reset {},
        ] {
            assert_eq!(
                next_deadline(&event, accepted, deadline, end).unwrap(),
                deadline
            );
        }
        assert_eq!(
            next_deadline(&Event::Renew {}, accepted, deadline, end).unwrap(),
            end
        );
        assert!(next_deadline(&Event::Renew {}, deadline, deadline, end).is_err());
    }
    #[test]
    fn blocked_ack_obeys_remaining_deadline() {
        let (mut a, _b) = UnixStream::pair().unwrap();
        a.set_nonblocking(true).unwrap();
        loop {
            match a.write(&[0]) {
                Ok(1) => (),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                other => panic!("unexpected socket fill result: {other:?}"),
            }
        }
        a.set_nonblocking(false).unwrap();
        let start = Instant::now();
        assert!(
            send(
                &mut a,
                &Ack {
                    kind: "ack".into(),
                    sequence: 1,
                    accepted: true
                },
                start + Duration::from_millis(40)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(200));
    }
    #[test]
    fn socket_cleanup_preserves_replacement() {
        let name = format!(
            "dw-input-socket-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(name);
        fs::create_dir(&directory).unwrap();
        let path = directory.join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let guard = SocketPath::new(path.clone()).unwrap();
        drop(guard);
        assert!(!path.exists());
        drop(listener);
        let listener = UnixListener::bind(&path).unwrap();
        let guard = SocketPath::new(path.clone()).unwrap();
        let original = directory.join("original.sock");
        fs::rename(&path, &original).unwrap();
        let replacement = UnixListener::bind(&path).unwrap();
        drop(guard);
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_socket());
        drop(listener);
        drop(replacement);
        fs::remove_file(path).unwrap();
        fs::remove_file(original).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn fragmented_read_does_not_extend_deadline() {
        use std::io::Write;
        let (mut a, mut b) = UnixStream::pair().unwrap();
        let thread = std::thread::spawn(move || {
            for byte in [0, 0, 0, 4, 1, 2, 3, 4] {
                if b.write_all(&[byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        let start = Instant::now();
        let mut reader = FrameReader {
            deadline: Some(start + Duration::from_millis(80)),
            ..FrameReader::default()
        };
        loop {
            match reader.poll(&mut a, Instant::now() + Duration::from_millis(20)) {
                Ok(None) => (),
                Ok(Some(_)) => panic!("fragmented frame should have expired"),
                Err(_) => break,
            }
        }
        assert!(start.elapsed() < Duration::from_millis(200));
        drop(a);
        thread.join().unwrap();
    }
    #[test]
    fn partial_frame_survives_poll_timeout_without_reframing() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        let mut reader = FrameReader::default();
        b.write_all(&[0, 0]).unwrap();
        assert!(
            reader
                .poll(&mut a, Instant::now() + Duration::from_millis(20))
                .unwrap()
                .is_none()
        );
        let deadline = reader.deadline;
        assert!(
            reader
                .poll(&mut a, Instant::now() + Duration::from_millis(20))
                .unwrap()
                .is_none()
        );
        assert_eq!(reader.deadline, deadline);
        b.write_all(&[0, 2, b'{', b'}']).unwrap();
        assert!(
            reader
                .poll(&mut a, Instant::now() + Duration::from_millis(20))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            reader
                .poll(&mut a, Instant::now() + Duration::from_millis(20))
                .unwrap(),
            Some(b"{}".to_vec())
        );
        assert!(reader.deadline.is_none());
    }
    #[test]
    fn idle_poll_has_no_frame_deadline_until_first_byte() {
        let (mut a, _b) = UnixStream::pair().unwrap();
        let mut reader = FrameReader::default();
        assert!(
            reader
                .poll(&mut a, Instant::now() + Duration::from_millis(5))
                .unwrap()
                .is_none()
        );
        assert!(reader.deadline.is_none());
    }
    #[test]
    fn oversized_frame_rejected_before_body() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        b.write_all(&((MAX_INPUT + 1) as u32).to_be_bytes())
            .unwrap();
        assert!(
            FrameReader::default()
                .poll(&mut a, Instant::now() + Duration::from_millis(20))
                .is_err()
        );
    }
}
