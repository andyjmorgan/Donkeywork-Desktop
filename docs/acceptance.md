# M1 acceptance

## M1a required acceptance — CLI and daemon

- On an explicitly authorized X11 pilot, run describe -> screenshot -> click a known test target -> screenshot verifying the effect. Test keyboard input without replaying uncertain actions.
- Native 3840×2160 PNG decodes to reported dimensions; metadata identifies session/epoch/display/topology/snapshot and capture time. Capture is fresh, cursor policy explicit, binary bytes bounded separately from JSON. Test incompressible images and decoded allocation bounds.
- CLI changes actual mode 3840×2160 -> 1920×1080 -> 3840×2160 with independently verified OS modes, same desktop session and uninterrupted PTY. Corner-pixel accuracy, stale topology, out-of-range coordinates, unsupported mode and failed rollback are tested.
- Explicit UID allowlist rejects unmapped callers; view-only cannot inject input or resize. Check forged identity, socket permissions, lease expiry/revoke, process cleanup and no sensitive content in logs.
- PTY supports resize, Ctrl-C, job control, correct exit and bounded reconnect policy under a fixed authorized OS profile.
- CLI emits structured results, nonzero failures and private screenshot files. Tests use synthetic non-sensitive desktop content; screenshots are sensitive artifacts, not audit logs.
- No loopback mock can satisfy actual desktop acceptance. No 4K streaming/browser-performance claim follows from PNG capture. Lab display changes need separately scoped approval.

## M1b required acceptance — browser and remote authentication

The following browser/streaming criteria apply to M1b, not to the local-only M1a slice.

No measurements exist yet. 3840×2160 native viewing and usable text are required; 60 fps is a target on a declared capable baseline. A proposed p95 input-to-photon LAN target is <100 ms, subject to Andrew's review and a measurement method that includes capture/encode/network/decode/render. Do not report software timestamp deltas as complete input-to-photon measurements.

## Required evidence

- A browser on another machine controls the actual pilot console and a PTY.
- Named host/OS/display stack, GPU/encoder, browser/client OS, codec/profile/chroma, resolution and network conditions.
- Small coloured terminal/IDE text, thin lines, scrolling, dragging, browser interaction and motion workloads.
- Capture, encoded and presented dimensions; no hidden resolution reduction.
- Sustained frame pacing, bandwidth, queue age, CPU/GPU and memory over a representative 30-minute run.
- Loss/delay experiments (proposed 40/80 ms added RTT and 0.5/1% loss), with recovery/stale-frame evidence.
- Decoder failure, stream reconfiguration and disconnect release input correctly.
- Keycloak expiry/wrong audience/unauthorized attachment/grant replay/revocation tests.
- Terminal resize, Ctrl-C, job control, process exit, bounded replay, gaps and authorized reconnect.
- Clipboard functionality without content in logs.
- Installation/start/stop behavior without disruption to unrelated workloads.
- Dependencies/provenance documented; outbound licence resolved before releases.

## Not sufficient

Schema tests, mocked online status, screenshots of the UI, a peak fps number, unmeasured “hardware accelerated” claims, loopback-only demos, disabled browser sandbox, unauthenticated debug endpoints, or a fallback that silently changes resolution.

4:4:4 is not lossless. If pixel-exact convergence is claimed, compare native-scale output with the captured reference under a specified colour pipeline. Hybrid refinement needs a separate contract amendment.

## Mandatory active-session resolution test

From the remote UI, change the actual target mode 3840×2160 -> 1920×1080 -> 3840×2160. Verify independently reported host modes, unchanged session identity/epoch, continued terminal output, generation/keyframe reset and correct pointer coordinates. Exercise unauthorized requests, stale topology, unsupported mode, driver failure and repeated requests; fail visibly without silently substituting browser scaling. The success path may briefly pause video, but must not require disconnect/login/reopening the session.
