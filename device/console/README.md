# Physical-console stream POC

This is the first physical-console provider. It captures the active DRM/KMS scanout beneath GDM/GNOME with independently MIT-licensed `libdrmtap`, encodes H.264 Annex B, and exposes it only through a root-owned local Unix socket. It never creates a desktop and never uses the managed `dwdesktop` Xorg display.

Current security/scope is intentionally narrow: the daemon and CLI run as root, the socket parent is root-owned `0700`, and the socket is `0600`. There is no network listener. This proves capture/encode before the privileged export and unprivileged conversion processes are split. Do not expose the socket through HTTP.

Build dependencies are libdrm, EGL/GLES2 headers, libcap, libseccomp and a C compiler because `libdrmtap` embeds its C implementation. Runtime needs an allowlisted DRM primary node and FFmpeg. The first encoder is `libx264`; `h264_nvenc` is a selectable experiment, not an automatic or proven fallback.

```sh
cargo test --manifest-path device/console/Cargo.toml
sudo install -d -o root -g root -m 0700 /run/dwconsole
sudo device/console/target/release/dwconsole-daemon \
  --socket /run/dwconsole/stream.sock \
  --device /dev/dri/card0 --encoder libx264

sudo device/console/target/release/dwconsole \
  --socket /run/dwconsole/stream.sock --seconds 3 \
  --h264 /var/lib/dwconsole/tap.h264 --png /var/lib/dwconsole/tap.png
```

The CLI validates root ownership/permissions, records the bounded stream in an exclusively created `0600` file, verifies Annex-B start codes and optionally decodes the first frame through FFmpeg into an exclusive private PNG. This is a stream tap, not a latency/4K/WebRTC acceptance result.

Known limitations: one client at a time; one active CRTC unless selected explicitly; XRGB/ARGB8888 mapped output only; reconnect on topology change; cursor plane is not composed; no audio, `uinput`, WebRTC, mode setting, daemon privilege split or fleet packaging yet. Failed capabilities are errors, not silent fallbacks.

Provenance: `libdrmtap` 0.5.4 is MIT licensed and consumed from crates.io. No RustDesk or JetKVM implementation source is copied. JetKVM informed the later H.264/WebRTC design only.
