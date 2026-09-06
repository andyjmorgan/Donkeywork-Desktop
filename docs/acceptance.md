# M1 acceptance

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
