# Linux PTY backend — WP05 component

Original standalone Rust library for real local terminals. No RustDesk code,
assets or translated implementation. Source baseline: main
`8c86296` (including the first CLI slice); wire semantics derive from the local
`dwdesktop.local` 0.2.0 contract. This library has no socket, JSON or auth layer.

## API and integration boundary

```rust,no_run
use donkeywork_desktop_terminal::{Pty, ServiceProfile, Size};
use std::{path::Path, time::Duration};

// Values come from administrator-selected service policy, never wire fields.
let profile = ServiceProfile::current_uid(1000, Path::new("/srv/desktop-work"))?;
let mut terminal = Pty::create(&profile, Size { rows: 24, cols: 80 })?;
terminal.write(1, b"pwd\n")?;
terminal.resize(1, Size { rows: 40, cols: 120 })?;
let batch = terminal.read(0, Duration::from_millis(100))?;
terminal.detach();
let resumed = terminal.resume(batch.latest_output_sequence)?;
terminal.close()?;
# Ok::<(), donkeywork_desktop_terminal::Error>(())
```

The dispatcher must enforce verified UID policy, session/epoch ownership,
permissions and one PTY per session. It owns terminal IDs and maps them to these
objects. `create` begins attached; detach on connection loss, `resume` under new
authorization, and `close` on revoke or session closure. Competing resumes fail.
The library never trusts caller-supplied identity or authorizes its own caller.

`ServiceProfile` has private fields. Its only constructor validates that the
expected UID matches both real and effective daemon UID. The current alpha
cannot switch users: mismatch returns `ProfileUidMismatch` before spawn. The
service chooses an absolute working directory; passwd supplies HOME/USER.
The executable is fixed `/bin/bash --noprofile --norc -i`. Environment is cleared
and populated with fixed PATH/TERM/LANG plus account identity, disabled history
file and a fixed prompt. No caller-selected executable, arguments or environment.

`openpty`, a dedicated session and a controlling tty supply canonical terminal
input, native job control and Ctrl-C/Ctrl-Z. `write` accepts raw bytes up to
32,768; protocol base64 decoding/canonical validation stays in the dispatcher.
Input sequence is contiguous from one, resize sequence independently increases.
A sequence accepted before a partial I/O failure remains consumed; never retry
unacknowledged bytes automatically. Writes wait at most 100 ms for buffer space.

The monitor reads independently of viewers. Replay retains at most 4 MiB, 4096
entries and 120 seconds, with each output chunk at most 32 KiB. It detaches a
lagging viewer and exposes a gap on resume; there is no complete terminal-emulator
state reconstruction. `read(after, wait)` returns a bounded batch and sequence
baseline, with exit only after final output. `resume` atomically snapshots replay
and input/resize baselines before the next read switches to live output. The
caller must deliver each returned batch before advancing its acknowledged
sequence; the library is not a network backpressure queue.

A background monitor enforces detached expiry even with no further API calls.
Explicit close/drop kills all discoverable foreground/background process groups
in the dedicated SID, closes the tty and reaps the shell. No raw output derives
Debug or enters diagnostics.

## Checks

```sh
cargo test --locked --manifest-path device/terminal/Cargo.toml
cargo clippy --locked --manifest-path device/terminal/Cargo.toml --all-targets -- -D warnings
```

Tests spawn actual local PTYs with fixed synthetic commands: controlling tty,
resize, split UTF-8, Ctrl-C, Ctrl-Z/foreground restoration, exit, background group
cleanup, sequence rejection, replay/resume and over-4-MiB output. Private-clock
tests exercise the 120-second watchdog/history expiry without waiting two minutes.
These are real local PTY tests, not Spark/session-wire integration tests.

## Limits and follow-up

- No core wiring, CLI interactive terminal, browser adapter, account switching or
  Spark deployment is included. Desktop-resolution/PTY continuity must still be
  demonstrated after integration.
- PTY process groups are not an isolation boundary. Programs that deliberately
  escape the session using `setsid` are outside this cleanup mechanism; stronger
  containment requires a separately designed cgroup/runtime policy.
- `/proc` must be available for discovery of job-control process groups. As with
  ordinary Unix process cleanup, a task stuck in uninterruptible kernel I/O can
  delay reaping. This is not a guarantee against a hostile same-UID workload.
- This initial service profile disables shell startup files/history and omits
  display/agent/credential environment variables. Further fixed profile options
  need review; wire requests cannot supply them.

Direct dependency `nix` is MIT; `tempfile` (MIT/Apache-2.0) is test-only.
Cargo.lock pins transitive packages. The project's outbound license is undecided.
