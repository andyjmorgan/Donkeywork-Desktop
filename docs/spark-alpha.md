# Spark alpha execution log

User authorized direct integration, build/deploy to Spark, real CLI E2E and restarts as needed on 2026-09-06. PR workflow resumes after proof. Only the integrator controls live host changes.

## Inventory before deployment

- Host office-spark-1, 192.168.69.28, ARM64 Ubuntu, localuser UID1000.
- Actual logged-in console is GNOME **Wayland**, not the historical X11 assumption. Xwayland is rootless :0; do not treat it as full console capture.
- Exposed Xwayland output None-1 is 1920×1080; no 3840×2160 mode in its advertised list.
- DRM inventory shows simple-framebuffer card0/Unknown-1. Existing xorg.conf requests NVIDIA; actual viable X11 driver/output mode setup still needs verification.
- k3s-agent and an existing Sunshine user service are active. No reboot or display-manager restart performed during this inventory.
- Original acceptance remains actual console capture/mode change, not a silently substituted virtual test display.

Native ARM64 Rust toolchain/build staging is scoped to /home/localuser/source/Donkeywork-Desktop. No public listener, firewall rule or Keycloak change is required for this local-socket alpha.

## First native execution

- Installed minimal Rust1.98.1 under localuser on Spark; compiled CLI, capture and core natively.
- 45 component tests passed on ARM64: CLI11, capture22, core12. Tests use synthetic frames and local Unix fixtures; not physical capture evidence.
- Started transient user service dwdesktop-alpha with view-only UID1000 policy and existing Xauthority supplied by environment; no xhost access widening.
- CLI describe successfully traversed actual Unix socket, peer UID policy, daemon adapter and Xwayland RandR enumeration. Reported display33, 1920×1080, no4K mode. This is an integration handshake, not proof of native console capture or usable live resize.
- Initial startup correctly rejected group-writable copied config; deployment corrected it to0600. Future deploy must install config with explicit private permissions and wait for socket readiness before calls.
- No screenshot/input/display mutation/reboot yet. User decision requested on a dedicated headless X11 desktop if the physical console cannot support the initial4K proof safely.
