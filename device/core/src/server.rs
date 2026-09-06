use crate::{
    core::{Connection, Core},
    wire,
};
use std::{
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{Semaphore, watch},
    task::JoinSet,
};

pub fn effective_uid() -> u32 {
    // getuid-style calls have no memory preconditions.
    unsafe { libc::geteuid() }
}
pub fn validate_directory(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.uid() != effective_uid() || metadata.mode() & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe socket directory",
        ));
    }
    Ok(())
}
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
    inode: u64,
    core: Arc<Mutex<Core>>,
}
impl Server {
    pub fn bind(path: &Path, core: Core) -> io::Result<Self> {
        validate_directory(
            path.parent()
                .ok_or_else(|| io::Error::other("socket needs parent"))?,
        )?;
        // Never unlink an existing file/socket. Administrators handle stale socket cleanup explicitly.
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        let inode = std::fs::symlink_metadata(path)?.ino();
        Ok(Self {
            listener,
            path: path.into(),
            inode,
            core: Arc::new(Mutex::new(core)),
        })
    }
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) -> io::Result<()> {
        let mut jobs = JoinSet::new();
        let capacity = Arc::new(Semaphore::new(16));
        let mut tick = tokio::time::interval(Duration::from_millis(25));
        loop {
            tokio::select! {
                _ = shutdown.changed() => { break; }
                _ = tick.tick() => { if let Ok(mut core) = self.core.try_lock() { core.maintain(std::time::Instant::now()); } }
                Some(_) = jobs.join_next(), if !jobs.is_empty() => {}
                accepted = self.listener.accept() => {
                    let (stream, _) = accepted?;
                    let uid = stream.peer_cred()?.uid();
                    let permit = match capacity.clone().try_acquire_owned() { Ok(p) => p, Err(_) => { drop(stream); continue; } };
                    let authorized = self.core.lock().map_err(|_| io::Error::other("worker unavailable"))?.authorized(uid);
                    if !authorized { drop(stream); continue; }
                    let core = self.core.clone(); let shutdown = shutdown.clone();
                    jobs.spawn(async move { let _permit = permit; handle(stream, uid, core, shutdown).await; });
                }
            }
        }
        while jobs.join_next().await.is_some() {}
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        // Remove only the socket inode created by this instance, never a replacement.
        if std::fs::symlink_metadata(&self.path).is_ok_and(|m| m.ino() == self.inode) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn handle(
    mut stream: UnixStream,
    uid: u32,
    core: Arc<Mutex<Core>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let connection = Connection::new(uid);
    // Duplicate the descriptor so the monitor cannot observe a reused/closed fd. It never consumes bytes.
    let monitor_fd = {
        // F_DUPFD_CLOEXEC returns an owned descriptor or -1; no borrowed pointer is involved.
        let fd = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
        if fd < 0 {
            return;
        }
        use std::os::fd::{FromRawFd, OwnedFd};
        unsafe { OwnedFd::from_raw_fd(fd) }
    };
    let cancelled = connection.cancelled.clone();
    let mut monitor_shutdown = shutdown.clone();
    let monitor = tokio::spawn(async move {
        loop {
            tokio::select! { _ = monitor_shutdown.changed() => break, _ = tokio::time::sleep(Duration::from_millis(10)) => {} }
            let mut byte = 0u8;
            // Peek checks peer closure during backend work without consuming request bytes.
            let result = unsafe {
                libc::recv(
                    monitor_fd.as_raw_fd(),
                    (&mut byte as *mut u8).cast(),
                    1,
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                )
            };
            if result == 0 {
                break;
            }
            if result < 0
                && !matches!(
                    io::Error::last_os_error().kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                )
            {
                break;
            }
        }
        cancelled.store(true, Ordering::Release);
    });
    loop {
        let request = tokio::select! {
            _ = shutdown.changed() => break,
            result = tokio::time::timeout(Duration::from_secs(30), wire::read_request(&mut stream)) => match result { Ok(Ok(request)) => request, _ => break },
        };
        let shared = core.clone();
        let caller = connection.clone();
        let reply = match tokio::task::spawn_blocking(move || {
            shared.lock().ok().map(|mut c| c.process(&caller, request))
        })
        .await
        {
            Ok(Some(reply)) => reply,
            _ => break,
        };
        if connection.cancelled.load(Ordering::Acquire) {
            break;
        }
        if !matches!(
            tokio::time::timeout(
                Duration::from_secs(5),
                wire::write_reply(&mut stream, &reply)
            )
            .await,
            Ok(Ok(()))
        ) {
            break;
        }
    }
    connection.cancelled.store(true, Ordering::Release);
    monitor.abort();
    if let Ok(mut worker) = core.lock() {
        worker.disconnect(&connection);
    }
}
