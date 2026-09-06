//! Original Linux PTY backend. Core must authenticate/authorize callers before
//! invoking this API; no client-provided executable, account or environment.
use nix::{
    pty::{openpty, Winsize},
    sys::signal::{killpg, Signal},
    unistd::{geteuid, getuid, Pid, Uid, User},
};
use std::{
    collections::{BTreeSet, VecDeque},
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::process::{CommandExt, ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const CHUNK_LIMIT: usize = 32_768;
pub const REPLAY_LIMIT: usize = 4 * 1024 * 1024;
pub const RETENTION: Duration = Duration::from_secs(120);
const REPLAY_ENTRY_LIMIT: usize = 4096;
const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    ProfileUidMismatch,
    InvalidProfile,
    InvalidSize,
    InvalidSequence,
    InvalidInput,
    NotAttached,
    ControlConflict,
    Closed,
    Io,
    OutputFailed,
}
pub type Result<T> = std::result::Result<T, Error>;

/// Construct only from administrator-selected configuration, never wire fields.
/// This alpha cannot change UID; mismatch fails before opening a PTY/spawning.
pub struct ServiceProfile {
    uid: u32,
    cwd: PathBuf,
    home: PathBuf,
    name: String,
}
impl ServiceProfile {
    pub fn current_uid(expected_uid: u32, working_directory: &Path) -> Result<Self> {
        if getuid().as_raw() != expected_uid || geteuid().as_raw() != expected_uid {
            return Err(Error::ProfileUidMismatch);
        }
        if !working_directory.is_absolute() || !working_directory.is_dir() {
            return Err(Error::InvalidProfile);
        }
        let user = User::from_uid(Uid::from_raw(expected_uid))
            .map_err(|_| Error::InvalidProfile)?
            .ok_or(Error::InvalidProfile)?;
        Ok(Self {
            uid: expected_uid,
            cwd: working_directory.into(),
            home: user.dir,
            name: user.name,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}
impl Size {
    fn winsize(self) -> Result<Winsize> {
        if self.rows == 0 || self.rows > 500 || self.cols == 0 || self.cols > 1000 {
            return Err(Error::InvalidSize);
        }
        Ok(Winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        })
    }
}

/// Raw bytes intentionally do not implement Debug to avoid accidental logs.
#[derive(Clone)]
pub struct Output {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<u8>,
    pub signal: Option<u8>,
    pub final_output_sequence: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub earliest_available_sequence: u64,
    pub latest_sequence: u64,
}
pub struct Batch {
    pub output: Vec<Output>,
    pub gap: Option<Gap>,
    pub exit: Option<Exit>,
    pub latest_output_sequence: u64,
}
pub struct Resumed {
    pub batch: Batch,
    pub last_accepted_input_sequence: u64,
    pub last_resize_sequence: u64,
}

struct Entry {
    output: Output,
    at: Instant,
}
struct State {
    child: Child,
    master: Option<File>,
    pid: i32,
    history: VecDeque<Entry>,
    history_bytes: usize,
    latest: u64,
    last_input: u64,
    last_resize: u64,
    last_delivered: u64,
    attached: bool,
    detached: Option<Instant>,
    stop: bool,
    exit: Option<Exit>,
    output_failed: bool,
}
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}
pub struct Pty {
    shared: Arc<Shared>,
    monitor: Option<JoinHandle<()>>,
}

impl Pty {
    pub fn create(profile: &ServiceProfile, size: Size) -> Result<Self> {
        if geteuid().as_raw() != profile.uid || getuid().as_raw() != profile.uid {
            return Err(Error::ProfileUidMismatch);
        }
        let pair = openpty(Some(&size.winsize()?), None).map_err(|_| Error::Io)?;
        let slave = File::from(pair.slave);
        let master = File::from(pair.master);
        for fd in [slave.as_raw_fd(), master.as_raw_fd()] {
            if unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFD, nix::libc::FD_CLOEXEC) } < 0 {
                return Err(Error::Io);
            }
        }
        // Master is nonblocking so slow terminal consumers/producers cannot pin
        // the daemon dispatcher. Child slave remains blocking and canonical.
        let flags = unsafe { nix::libc::fcntl(master.as_raw_fd(), nix::libc::F_GETFL) };
        if flags < 0
            || unsafe {
                nix::libc::fcntl(
                    master.as_raw_fd(),
                    nix::libc::F_SETFL,
                    flags | nix::libc::O_NONBLOCK,
                )
            } < 0
        {
            return Err(Error::Io);
        }
        let mut command = Command::new("/bin/bash");
        command
            .args(["--noprofile", "--norc", "-i"])
            .current_dir(&profile.cwd)
            .env_clear()
            .env("HOME", &profile.home)
            .env("USER", &profile.name)
            .env("LOGNAME", &profile.name)
            .env("PATH", "/usr/bin:/bin")
            .env("TERM", "xterm-256color")
            .env("LANG", "C.UTF-8")
            .env("HISTFILE", "/dev/null")
            .env("PS1", "dwdesktop$ ")
            .env("PS2", "> ")
            .stdin(Stdio::from(slave.try_clone().map_err(|_| Error::Io)?))
            .stdout(Stdio::from(slave.try_clone().map_err(|_| Error::Io)?))
            .stderr(Stdio::from(slave));
        // Only async-signal-safe libc calls run in the forked child. Stdio has
        // already been installed by std before this hook. Creating a session
        // and controlling tty enables native shell foreground job control.
        unsafe {
            command.pre_exec(|| {
                if nix::libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if nix::libc::ioctl(0, nix::libc::TIOCSCTTY, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().map_err(|_| Error::Io)?;
        let pid = child.id() as i32;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                child,
                master: Some(master),
                pid,
                history: VecDeque::new(),
                history_bytes: 0,
                latest: 0,
                last_input: 0,
                last_resize: 0,
                last_delivered: 0,
                attached: true,
                detached: None,
                stop: false,
                exit: None,
                output_failed: false,
            }),
            changed: Condvar::new(),
        });
        let state = shared.clone();
        let monitor = thread::Builder::new()
            .name("desktop-pty".into())
            .spawn(move || monitor(state))
            .map_err(|_| {
                let mut state = shared.state.lock().unwrap();
                terminate(&mut state);
                Error::Io
            })?;
        Ok(Self {
            shared,
            monitor: Some(monitor),
        })
    }

    pub fn process_id(&self) -> u32 {
        self.shared.state.lock().unwrap().pid as u32
    }

    /// One bounded chunk. An accepted sequence is consumed even if the OS write
    /// is partial; callers must never blindly resend after an error.
    pub fn write(&self, sequence: u64, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() || bytes.len() > CHUNK_LIMIT {
            return Err(Error::InvalidInput);
        }
        let mut s = self.shared.state.lock().unwrap();
        check_attached(&s)?;
        if sequence != s.last_input + 1 || sequence > MAX_SEQUENCE {
            return Err(Error::InvalidSequence);
        }
        s.last_input = sequence;
        let deadline = Instant::now() + Duration::from_millis(100);
        let mut remaining = bytes;
        while !remaining.is_empty() {
            match s.master.as_mut().ok_or(Error::Closed)?.write(remaining) {
                Ok(0) => return Err(Error::Io),
                Ok(n) => remaining = &remaining[n..],
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(_) => return Err(Error::Io),
            }
        }
        Ok(())
    }

    pub fn resize(&self, sequence: u64, size: Size) -> Result<()> {
        let winsize = size.winsize()?;
        let mut s = self.shared.state.lock().unwrap();
        check_attached(&s)?;
        if sequence <= s.last_resize || sequence > MAX_SEQUENCE {
            return Err(Error::InvalidSequence);
        }
        s.last_resize = sequence;
        let fd = s.master.as_ref().ok_or(Error::Closed)?.as_raw_fd();
        if unsafe { nix::libc::ioctl(fd, nix::libc::TIOCSWINSZ, &winsize) } < 0 {
            return Err(Error::Io);
        }
        Ok(())
    }

    /// Wait at most the requested duration (capped to one second), then return a
    /// bounded batch. Exit is emitted only alongside/after all final output.
    pub fn read(&self, after: u64, wait: Duration) -> Result<Batch> {
        let mut s = self.shared.state.lock().unwrap();
        expire_history(&mut s, Instant::now());
        if after > s.latest {
            return Err(Error::InvalidSequence);
        }
        if !s.attached {
            return Err(Error::NotAttached);
        }
        if s.output_failed {
            return Err(Error::OutputFailed);
        }
        if after == s.latest && s.exit.is_none() {
            s = self
                .shared
                .changed
                .wait_timeout(s, wait.min(Duration::from_secs(1)))
                .unwrap()
                .0;
        }
        if !s.attached {
            return Err(Error::NotAttached);
        }
        if s.output_failed {
            return Err(Error::OutputFailed);
        }
        Ok(batch(&mut s, after))
    }

    /// Core calls this when its authenticated attachment is lost. Retention is
    /// enforced by the monitor even when the API is otherwise idle.
    pub fn detach(&self) {
        let mut s = self.shared.state.lock().unwrap();
        if s.attached {
            s.attached = false;
            s.detached = Some(Instant::now());
        }
        self.shared.changed.notify_all();
    }
    pub fn resume(&self, after: u64) -> Result<Resumed> {
        let mut s = self.shared.state.lock().unwrap();
        if s.attached {
            return Err(Error::ControlConflict);
        }
        if s.stop || s.detached.is_some_and(|t| t.elapsed() >= RETENTION) {
            return Err(Error::Closed);
        }
        if after > s.latest {
            return Err(Error::InvalidSequence);
        }
        expire_history(&mut s, Instant::now());
        s.attached = true;
        s.detached = None;
        let result = Resumed {
            last_accepted_input_sequence: s.last_input,
            last_resize_sequence: s.last_resize,
            batch: batch(&mut s, after),
        };
        Ok(result)
    }

    /// Explicit revoke/session close: terminate all discoverable process groups
    /// in this dedicated terminal session, close the tty and reap the shell.
    pub fn close(&mut self) -> Result<()> {
        {
            let mut s = self.shared.state.lock().unwrap();
            s.stop = true;
        }
        self.shared.changed.notify_all();
        if let Some(monitor) = self.monitor.take() {
            monitor.join().map_err(|_| Error::Io)?;
        }
        if self.shared.state.lock().unwrap().output_failed {
            return Err(Error::OutputFailed);
        }
        Ok(())
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn check_attached(s: &State) -> Result<()> {
    if s.stop || s.exit.is_some() {
        return Err(Error::Closed);
    }
    if !s.attached {
        return Err(Error::NotAttached);
    }
    Ok(())
}
fn expire_history(s: &mut State, now: Instant) {
    while s
        .history
        .front()
        .is_some_and(|e| now.duration_since(e.at) >= RETENTION)
        || s.history_bytes > REPLAY_LIMIT
        || s.history.len() > REPLAY_ENTRY_LIMIT
    {
        if let Some(entry) = s.history.pop_front() {
            s.history_bytes -= entry.output.bytes.len();
        } else {
            break;
        }
    }
    if s.attached
        && s.history
            .front()
            .map_or(s.latest + 1, |e| e.output.sequence)
            > s.last_delivered + 1
    {
        s.attached = false;
        s.detached = Some(now);
    }
}
fn batch(s: &mut State, after: u64) -> Batch {
    let earliest = s
        .history
        .front()
        .map_or(s.latest + 1, |e| e.output.sequence);
    let gap = (after + 1 < earliest).then_some(Gap {
        earliest_available_sequence: earliest,
        latest_sequence: s.latest,
    });
    s.last_delivered = s.latest;
    Batch {
        output: s
            .history
            .iter()
            .filter(|e| e.output.sequence > after)
            .map(|e| e.output.clone())
            .collect(),
        gap,
        exit: s.exit,
        latest_output_sequence: s.latest,
    }
}
fn drain(s: &mut State) {
    let mut buffer = [0; CHUNK_LIMIT];
    for _ in 0..64 {
        let Some(master) = s.master.as_mut() else {
            return;
        };
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if s.latest == MAX_SEQUENCE {
                    s.output_failed = true;
                    s.stop = true;
                    break;
                }
                s.latest += 1;
                s.history_bytes += n;
                s.history.push_back(Entry {
                    output: Output {
                        sequence: s.latest,
                        bytes: buffer[..n].into(),
                    },
                    at: Instant::now(),
                });
                expire_history(s, Instant::now());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.raw_os_error() == Some(nix::libc::EIO) => break,
            Err(_) => {
                s.output_failed = true;
                s.stop = true;
                break;
            }
        }
    }
}
fn monitor(shared: Arc<Shared>) {
    loop {
        let mut s = shared.state.lock().unwrap();
        drain(&mut s);
        expire_history(&mut s, Instant::now());
        if s.stop || s.detached.is_some_and(|t| t.elapsed() >= RETENTION) {
            s.stop = true;
            terminate(&mut s);
            shared.changed.notify_all();
            return;
        }
        if s.exit.is_some() {
            shared.changed.notify_all();
            drop(s);
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        match s.child.try_wait() {
            Ok(Some(status)) => {
                kill_session_groups(s.pid);
                // Descendants are stopped, so remaining kernel PTY output is
                // finite and below the replay bound. Drain before announcing exit.
                drain(&mut s);
                s.master.take();
                s.exit = Some(Exit {
                    code: status.code().map(|c| c as u8),
                    signal: status.signal().map(|v| v as u8),
                    final_output_sequence: s.latest,
                });
                shared.changed.notify_all();
                continue;
            }
            Ok(None) => {}
            Err(_) => {
                s.output_failed = true;
                s.stop = true;
                terminate(&mut s);
                shared.changed.notify_all();
                return;
            }
        }
        shared.changed.notify_all();
        drop(s);
        thread::sleep(Duration::from_millis(10));
    }
}

fn kill_session_groups(session: i32) {
    // Foreground/background jobs use separate process groups. Killing only the
    // shell's group would leave jobs behind. Scan only this dedicated SID.
    let mut groups = BTreeSet::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
                continue;
            }
            let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            let Some((_, rest)) = stat.rsplit_once(") ") else {
                continue;
            };
            let fields: Vec<_> = rest.split_whitespace().take(4).collect();
            if fields.len() == 4 && fields[3].parse::<i32>().ok() == Some(session) {
                if let Ok(group) = fields[2].parse::<i32>() {
                    if group > 0 && group != session {
                        groups.insert(group);
                    }
                }
            }
        }
    }
    for group in groups {
        let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
    }
    let _ = killpg(Pid::from_raw(session), Signal::SIGKILL);
}
fn terminate(s: &mut State) {
    if s.exit.is_some() {
        s.master.take();
        return;
    }
    kill_session_groups(s.pid);
    let _ = s.child.kill();
    match s.child.wait() {
        Ok(status) => {
            drain(s);
            s.master.take();
            s.exit = Some(Exit {
                code: status.code().map(|c| c as u8),
                signal: status.signal().map(|v| v as u8),
                final_output_sequence: s.latest,
            });
        }
        Err(_) => {
            s.output_failed = true;
            s.master.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal() -> (tempfile::TempDir, Pty) {
        let dir = tempfile::tempdir().unwrap();
        let profile = ServiceProfile::current_uid(geteuid().as_raw(), dir.path()).unwrap();
        let pty = Pty::create(&profile, Size { rows: 24, cols: 80 }).unwrap();
        (dir, pty)
    }

    #[test]
    fn disconnected_deadline_terminates_without_further_api_calls() {
        let (_dir, mut pty) = terminal();
        let pid = pty.process_id();
        // Advance only the private test timestamp; production retention is fixed.
        {
            let mut s = pty.shared.state.lock().unwrap();
            s.attached = false;
            s.detached = Some(Instant::now() - RETENTION - Duration::from_secs(1));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Path::new(&format!("/proc/{pid}")).exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        assert!(matches!(pty.resume(0), Err(Error::Closed)));
        pty.close().unwrap();
    }

    #[test]
    fn expired_history_is_dropped_and_gap_remains_explicit() {
        let (_dir, mut pty) = terminal();
        {
            let mut s = pty.shared.state.lock().unwrap();
            s.attached = false;
            s.detached = Some(Instant::now());
            s.history.clear();
            s.history_bytes = 1;
            s.latest = 1;
            s.history.push_back(Entry {
                output: Output {
                    sequence: 1,
                    bytes: vec![0],
                },
                at: Instant::now() - RETENTION - Duration::from_secs(1),
            });
            expire_history(&mut s, Instant::now());
            assert!(s.history.is_empty());
            assert_eq!(s.history_bytes, 0);
            let batch = batch(&mut s, 0);
            assert_eq!(
                batch.gap,
                Some(Gap {
                    earliest_available_sequence: 2,
                    latest_sequence: 1
                })
            );
        }
        pty.close().unwrap();
    }
}
