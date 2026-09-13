# Physical console browser prototype — 2026-09-06

Current target: show Spark's physical console in a browser at its private IP.
Input implementation is deliberately sequenced after a separate RustDesk and
JetKVM source review requested by Andrew. This page is a working evidence log;
browser deployment/decoding are not yet marked passed.

Already verified: root-local DRM capture from `/dev/dri/card0`, active CRTC35,
1920×1080 XR24 linear scanout; CLI recorded and decoded the daemon's H.264
stream. This is the physical-console provider, separate from managed Xorg :99.
Continuing video decode alone will not prove the scanout responds to input;
that needs a later controlled visible change. 4K and mode switching are not
validated on this fixed 1080p source.

The display adapter contract is in `contracts/console-web-prototype.md`.
`web-launch` connects the private socket then clears groups and drops to UID/GID
65534 before executing the Go bridge. The service binds HTTP192.168.69.28:8090
and WebRTC UDP192.168.69.28:8091. No firewall/router/tunnel changes are required.
The web prototype is unauthenticated by the user's requested scope and should
remain on the lab network.

Capture cleanup now kills/waits the encoder and joins the writer on capture
errors; CLI framing errors/timeouts close instead of retrying midway through a
record. Rust unit checks and strict clippy passed before Spark release build.
Full privilege separation of capture and encoder, blocked-pipe cancellation and
persistent startup packaging remain further work.

Browser verification command:

```sh
node tests/integration/live-console-smoke.mjs http://192.168.69.28:8090
```

This checks actual native-video dimensions and increasing decoded-frame counts
and saves a private 1280×800 screenshot. It sends no desktop input. Do not
upload physical-console screenshots to public storage without reviewing their
contents.
