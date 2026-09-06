use donkeywork_desktop_terminal::*;
use std::{
    fs, thread,
    time::{Duration, Instant},
};

struct Harness {
    pty: Pty,
    seq: u64,
    input: u64,
    dir: tempfile::TempDir,
}
impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let profile =
            ServiceProfile::current_uid(nix::unistd::geteuid().as_raw(), dir.path()).unwrap();
        let pty = Pty::create(&profile, Size { rows: 24, cols: 80 }).unwrap();
        let mut h = Self {
            pty,
            seq: 0,
            input: 0,
            dir,
        };
        h.until(b"dwdesktop$ ");
        h.send(b"stty -echo\n");
        h.until(b"dwdesktop$ ");
        h
    }
    fn send(&mut self, bytes: &[u8]) {
        self.input += 1;
        self.pty.write(self.input, bytes).unwrap();
    }
    fn until(&mut self, marker: &[u8]) -> Vec<u8> {
        let end = Instant::now() + Duration::from_secs(4);
        let mut bytes = Vec::new();
        while Instant::now() < end {
            let batch = self.pty.read(self.seq, Duration::from_millis(50)).unwrap();
            assert!(batch.gap.is_none());
            for chunk in batch.output {
                assert_eq!(chunk.sequence, self.seq + 1);
                self.seq = chunk.sequence;
                bytes.extend(chunk.bytes);
            }
            if bytes.windows(marker.len()).any(|w| w == marker) {
                return bytes;
            }
        }
        panic!("Timed out waiting for fixed synthetic test marker; PTY content suppressed");
    }
}

#[test]
fn real_controlling_tty_resize_utf8_and_exit() {
    let mut h = Harness::new();
    h.send(b"test -t 0 && test -t 1 && printf '__TTY__\\n'; stty size\n");
    let out = h.until(b"dwdesktop$ ");
    assert!(out.windows(7).any(|w| w == b"__TTY__"));
    assert!(out.windows(5).any(|w| w == b"24 80"));
    h.pty
        .resize(
            1,
            Size {
                rows: 42,
                cols: 100,
            },
        )
        .unwrap();
    h.send(b"stty size\n");
    let out = h.until(b"dwdesktop$ ");
    assert!(out.windows(6).any(|w| w == b"42 100"));
    h.send(b"printf '%s\\n' '");
    h.send(&[0xc3]);
    h.send(&[0xa9]);
    h.send(b"'\n");
    let out = h.until(b"dwdesktop$ ");
    assert!(out.windows(2).any(|w| w == [0xc3, 0xa9]));
    h.send(b"exit 7\n");
    let end = Instant::now() + Duration::from_secs(3);
    let mut exit = None;
    while Instant::now() < end {
        let b = h.pty.read(h.seq, Duration::from_millis(50)).unwrap();
        h.seq = b.latest_output_sequence;
        if b.exit.is_some() {
            exit = b.exit;
            break;
        }
    }
    let exit = exit.expect("real shell exit required");
    assert_eq!(exit.code, Some(7));
    assert_eq!(exit.signal, None);
    assert_eq!(exit.final_output_sequence, h.seq);
    h.pty.close().unwrap();
    h.pty.close().unwrap();
}

#[test]
fn ctrl_c_interrupts_foreground_job_and_shell_survives() {
    let mut h = Harness::new();
    h.send(b"sleep 30\n");
    thread::sleep(Duration::from_millis(100));
    h.send(&[3]);
    h.until(b"dwdesktop$ ");
    h.send(b"printf '__AFTER_INTERRUPT__\\n'\n");
    let out = h.until(b"dwdesktop$ ");
    assert!(out.windows(19).any(|w| w == b"__AFTER_INTERRUPT__"));
}

#[test]
fn ctrl_z_and_fg_use_native_job_control() {
    let mut h = Harness::new();
    h.send(b"sleep 30\n");
    thread::sleep(Duration::from_millis(100));
    h.send(&[26]);
    let out = h.until(b"dwdesktop$ ");
    assert!(out.windows(7).any(|w| w == b"Stopped"));
    h.send(b"fg\n");
    thread::sleep(Duration::from_millis(100));
    h.send(&[3]);
    h.until(b"dwdesktop$ ");
}

#[test]
fn invalid_profile_sequences_and_sizes_have_no_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let uid = nix::unistd::geteuid().as_raw();
    assert!(matches!(
        ServiceProfile::current_uid(uid.wrapping_add(1), dir.path()),
        Err(Error::ProfileUidMismatch)
    ));
    let mut h = Harness::new();
    assert_eq!(
        h.pty.write(h.input, b"printf bad\n"),
        Err(Error::InvalidSequence)
    );
    assert_eq!(
        h.pty.write(h.input + 2, b"printf bad\n"),
        Err(Error::InvalidSequence)
    );
    assert_eq!(
        h.pty.write(h.input + 1, &vec![0; CHUNK_LIMIT + 1]),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        h.pty.resize(1, Size { rows: 0, cols: 80 }),
        Err(Error::InvalidSize)
    );
    h.pty.resize(1, Size { rows: 25, cols: 80 }).unwrap();
    assert_eq!(
        h.pty.resize(1, Size { rows: 26, cols: 80 }),
        Err(Error::InvalidSequence)
    );
    h.send(b"printf '__VALID__\\n'\n");
    h.until(b"dwdesktop$ ");
}

#[test]
fn detach_resume_is_exclusive_and_reports_sequence_baselines() {
    let mut h = Harness::new();
    assert!(matches!(h.pty.resume(0), Err(Error::ControlConflict)));
    h.pty
        .resize(
            3,
            Size {
                rows: 25,
                cols: 100,
            },
        )
        .unwrap();
    h.send(b"printf '__REPLAY__\\n'\n");
    h.pty.detach();
    assert_eq!(h.pty.write(h.input + 1, b"x"), Err(Error::NotAttached));
    thread::sleep(Duration::from_millis(100));
    let resumed = h.pty.resume(h.seq).unwrap();
    assert_eq!(resumed.last_accepted_input_sequence, h.input);
    assert_eq!(resumed.last_resize_sequence, 3);
    assert!(resumed.batch.gap.is_none());
    assert!(!resumed.batch.output.is_empty());
    h.seq = resumed.batch.latest_output_sequence;
    assert!(matches!(
        h.pty.read(h.seq + 1, Duration::ZERO),
        Err(Error::InvalidSequence)
    ));
}

#[test]
fn high_output_replay_is_bounded_and_gap_is_explicit() {
    let mut h = Harness::new();
    h.send(b"head -c 5500000 /dev/zero | tr '\\0' x; printf done > completed\n");
    h.pty.detach();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !h.dir.path().join("completed").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(h.dir.path().join("completed").exists());
    thread::sleep(Duration::from_millis(50));
    let resumed = h.pty.resume(0).unwrap();
    assert!(resumed.batch.gap.is_some());
    assert!(
        resumed
            .batch
            .output
            .iter()
            .map(|v| v.bytes.len())
            .sum::<usize>()
            <= REPLAY_LIMIT
    );
    assert!(resumed
        .batch
        .output
        .iter()
        .all(|v| v.bytes.len() <= CHUNK_LIMIT));
}

#[test]
fn close_terminates_background_job_groups_and_reaps_shell() {
    let mut h = Harness::new();
    let shell = h.pty.process_id();
    h.send(b"sleep 30 & printf '%s' $! > background.pid\n");
    h.until(b"dwdesktop$ ");
    let pid: u32 = fs::read_to_string(h.dir.path().join("background.pid"))
        .unwrap()
        .parse()
        .unwrap();
    h.pty.close().unwrap();
    assert!(!std::path::Path::new(&format!("/proc/{shell}")).exists());
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => break,
            Ok(stat) if stat.rsplit_once(") ").unwrap().1.starts_with('Z') => break,
            _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => panic!("background job remained alive after close"),
        }
    }
}

#[test]
fn shell_environment_does_not_inherit_service_secrets_or_rc_files() {
    let mut h = Harness::new();
    h.send(b"test -z \"$BASH_ENV\" && test -z \"$LD_PRELOAD\" && test \"$HISTFILE\" = /dev/null && printf '__SANITIZED__\\n'\n");
    let out = h.until(b"dwdesktop$ ");
    assert!(out.windows(13).any(|w| w == b"__SANITIZED__"));
}
