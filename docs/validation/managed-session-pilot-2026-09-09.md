# Managed-session pilot: Spark and Minigpu

Date: 2026-09-09. Scope: CLI-driven authenticated managed-desktop proof, **not**
fleet web console completion or a production terminal-services release.

## Follow-up: managed web integration completed for the pilot

### Logout choice correction

The user logged out of both managed desktops. Previously this also stopped the
viewer. An initial automatic-recreation implementation was inappropriate: the
user clarified that starting another desktop must be a choice. That automatic
policy was removed from both pilots (`Restart=no`), without restarting their
physical consoles.

The portal now survives logout, reports `session_ended`, and asks whether to
start a desktop. Its only creation trigger is an explicit button POST (or CLI
create). An existing live desktop is reused, never replaced on reconnect.
`managed-session-choice.mjs` exercises normal scoped Xfce logout, waits with no
automatic recreation, clicks the start button, checks new PID/video/input, then
reloads and checks that the PID is preserved. It logs out again at the end to
leave the user's intended stopped state intact. The old auto-recreation test
was removed because it asserted the wrong policy.

The initial transition from transient to persistent user units required a
daemon-reload after the transient definition disappeared; the explicit start
path now handles that before starting the fixed unit. The persistent unit is
not enabled at boot. Portal HTTP remains available on 8095; the internal media
bridge listens on 8097 and retains the existing WebRTC UDP port 8096.

### Input-channel loss follow-up

On Minigpu, a closed browser input data channel left video playing and subsequent
acquisition displayed `Input unavailable on this connection`. The old handler
marked input unavailable but did not replace the closed connection. The original
closure trigger was not established from the available logs; this is a confirmed
recovery defect, not a claim that every possible transport failure is fixed.

Managed viewers now automatically renegotiate when their input channel closes.
The Python supervisor also replaces the encoder/bridge if its input worker exits,
without restarting the OS desktop. Deployed to Minigpu only for this incident.
`tests/integration/managed-input-recovery.mjs` deliberately closed the browser
data channel, verified a new open channel and resumed frames, then acquired and
released control without delivering desktop input. Passed. Managed desktop PID
883849 was unchanged before/after deployment. Frontend 73 tests and Python three
adapter tests passed. Existing browser tabs need one reload to load the fix.

The CLI-only limitations recorded later in this note describe the first test
pass. A subsequent implementation now connects the existing WebRTC UI to the
managed desktop on both hosts:

- `http://192.168.69.28:8095/?managed`
- `http://192.168.69.21:8095/?managed`

Actual Chromium tests on both hosts decoded live H.264, clicked the Xfce terminal
launcher, typed `echo webinputok`, scrolled terminal history, released held Shift
on blur, reacquired input, typed `echo releaseok`, reloaded/reconnected, and used
the web buttons to change 1080p → 4K → 1080p. Screenshots visibly show lowercase
`releaseok`, retained terminal windows and the remote command output. The final
test also reacquires control after the resize cycle.

Evidence: `artifacts/managed-web-proof/HOST/{report.json,typed.png,before-wheel.png,
after-wheel.png,released.png,reconnected.png,resized.png}`. The test uses an
actual browser/PeerConnection and normal mouse/keyboard events; it does not use
SSH app launches as input evidence. Screenshots are browser captures at 1280×900
of the scaled remote raster, not proof that the remote mode itself is 1280×900.

Input uses the original Rust X11 core, not the physical uinput helper. A small
draft-protocol extension adds explicit wheel detent click buttons with backend
rejection of held-wheel operations. Media resize restarts only encoder/bridge;
the OS desktop is retained. The UI automatically reconnects but this is not a
seamless video-generation transition. One test initially reacquired before the
release acknowledgement; it now waits for idle. A stale UI warning from the old
connection was also reset when creating the new input controller.

Regression results after integration: 3 Python adapter tests, 16 Rust core
tests, 73 frontend tests, the Go bridge suite and root contract suite pass.
The new web endpoint is **unauthenticated and lab-only**. Create/destroy remains
in the password-authenticated CLI. Secure web credential handoff, fleet auth,
general PAM graphical-login semantics and a production rollout are not claimed.

## Observed results

| Actual test | Spark .28, Ubuntu ARM64 | Minigpu .21, Ubuntu amd64 |
|---|---|---|
| Password-only SSH authentication, keys/agent disabled | Pass | Pass |
| Random invalid password, no command sent | Rejected | Rejected |
| Private Xorg dummy/Xfce as localuser | Pass, :109 | Pass, :109 |
| CLI screenshot, visually inspected desktop | Pass | Pass |
| Click terminal launcher, type diagnostic, press Enter | Pass | Pass |
| Rendered account and display | localuser, :109.0 | localuser, :109.0 |
| 1920×1080 → 3840×2160 → 1920×1080 | Pass | Pass |
| Same protocol session ID across resize | Pass | Pass |
| Close/open attachment, application remains | Pass | Pass |
| Native Firefox renders example.com | Pass | Pass |
| libx264, three seconds at 1080p30; decoder counts 90 frames | Pass | Pass |
| Decoded H.264 frame visually inspected | Pass | Pass |
| Interactive SSH PTY, detach/reattach retains PID and variable | Pass | Pass |
| Destroy: X109 socket, core socket and cgroup absent | Pass | Pass |
| Recreate from stopped state | Pass | Pass |
| GDM selected and active after all changes | Pass | Pass |
| Original physical user session stays active | No user console login baseline | Session 214 remains Active=yes |

Minigpu rejects duplicate `create` without replacing the running session.
Both pilots are left running a managed desktop at 1080p. Their existing physical
console services are not replaced. No reboot, GDM restart, cluster restart,
firewall change or public listener was performed for this pilot.

## Reproducible commands and evidence

See [the command guide](../../deploy/managed/README.md) and run
`python3 deploy/managed/verify.py HOST --browser` against a ready pilot with its
default Xfce dock visible. Input is sent using our existing Rust daemon/CLI.
The browser launch command is typed into the GUI terminal; this is not an SSH
process launch passed off as desktop input. The separate `terminal` test uses
the explicitly documented SSH/tmux path.

Final verification IDs:

- Spark: `22c2d85f-3e47-41ba-9c98-04c9c6e1770c`
- Minigpu: `52e0b645-dff7-40a9-8297-b9f71964ae97`

Full generated evidence is in `/tmp/dwmanaged-proof-HOST-ID` on the controller.
Retained copies under ignored `artifacts/managed-pilot/` include each report,
typed-input screenshots, Spark's fullscreen browser screenshot, and Minigpu's
decoded H.264 frame. These are local artifacts, not committed public images.
H.264 clips remain under each user's private managed runtime, named `ID.h264`.
The final Spark screenshot was taken after CLI-clicking Firefox's onboarding
Continue and sending F11; it displays Example Domain fullscreen at 1080p.

PTY reconnect evidence used `DW_PTY_PROOF=retained`. Spark retained shell PID
672803; Minigpu retained PID 873854 across actual CLI terminal detach/reattach.
Those PIDs belonged to earlier test instances and were terminated during the
successful destruction tests; they are not the current live shell PIDs.

Component regressions:

- `cargo test --locked --manifest-path device/core/Cargo.toml`: 15 passed.
- `cargo test --locked --manifest-path cli/Cargo.toml`: 14 passed.
- Python compile checks: passed.
- `git diff --check`: passed.

These checks do not constitute a soak test, internet-hardening audit or
latency/bandwidth benchmark. Encoding/decoding a 90-frame sample is not proof
of WebRTC delivery, browser frame pacing or high-motion quality.

## Failures found and changes made

1. Inherited SESSION_MANAGER from the user service environment caused Xfce to
   report another session manager and exit. The managed runtime removes it,
   Wayland display and physical-session identity variables; it starts its own
   D-Bus. A rendered desktop was then confirmed, not inferred from unit state.
2. Ubuntu's system autostart launched Light Locker on Minigpu. It failed because
   this unit is not a logind graphical session. Private per-desktop autostart
   overrides now suppress physical-seat helpers. The crash-report popup was
   observed, investigated, and absent on the subsequent clean desktop.
3. The initial apt operation accidentally included recommendations and installed
   LightDM, Light Locker, its settings UI, Unity greeter and Unity settings daemon.
   These five newly added packages were subsequently removed on both hosts after
   simulating removal. They can be reinstalled; conffiles were not purged. No
   broad autoremove was run because it also offered unrelated NVIDIA packages
   on Spark. Some inert recommended libraries remain installed. The documented
   deployment prerequisite uses `--no-install-recommends`.
4. One early Minigpu teardown left an Xfce panel awaiting systemd's stop timeout.
   The managed unit now uses a 10-second stop budget and cgroup cleanup. Later
   destroy tests returned promptly and verified cgroup/socket absence.
5. Minigpu Snap Firefox failed on the private bus with a snap-cgroup error. A
   separate native Mozilla build, checksum matched against published sums, runs
   successfully without changing the existing Snap. Spark's native build was
   reused. Native Minigpu Firefox prints a user-namespace EPERM diagnostic;
   browser sandbox hardening requires a separate review. No system user-namespace
   restriction was relaxed and no browser sandbox-disabling flag was added.

## What this establishes—and does not

We can reuse the Rust desktop core, X11 screenshot/input backend and RandR
resize path for an owned managed desktop. No physical monitor, KMS/VKMS,
greeter capture, physical-seat input guard or greeter/user handoff was involved.
Existing H.264 encoding works against this private display. The difficult
physical-console lifecycle is absent from this use case.

Authentication is real SSH/PAM authentication; the resulting graphical unit
is **not** a new display-manager/PAM graphical login. It does not yet supply
keyring unlocking or desktop polkit semantics. This matters when evolving
from a single-account pilot into general-purpose terminal services.

The terminal currently reuses SSH's PTY plus private tmux, not our unfinished
native terminal protocol. Managed browser streaming/input, secure web credential
handoff, multi-user lifecycle, authentication expiry/revocation and production
packaging remain further implementation. In particular, the old physical
console web endpoints are not URLs for these managed sessions.

The next bounded implementation is to connect the existing WebRTC renderer to
the managed H.264 stream and route browser input through the private X11 core,
then expose create/view/destroy. Do not reconnect physical uinput or greeter
guards merely to reuse an old web launcher.
