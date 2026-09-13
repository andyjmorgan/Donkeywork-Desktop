# Managed multi-desktop broker validation — 2026-09-09

## Delivered

Single-host broker on Spark `.28` and Minigpu `.21`, TCP 8095 `/?managed`.
GNOME (Ubuntu) and Xfce are selectable supported adapters. Inventory is an array;
each desktop has independent reconnect/end actions. Creation remains available
alongside existing desktops. Viewer loss does not implicitly create a desktop.
CLI `--desktop ID` selects an instance for native input/screenshot commands,
SSH/tmux terminal, status or scoped destruction.

## Browser proof

`node tests/integration/managed-broker-pilot.mjs HOST` passed on both hosts:

- Create/list concurrent GNOME and Xfce through the actual React broker.
- Reconnect each instance to the H.264 browser viewer.
- GNOME: click Activities, search Terminal, launch it and type an echo command
  using browser keyboard input. Inspected screenshots show the command and its
  output on both hosts; input ACK alone was not treated as proof.
- Each environment changed 1080p → 4K → 1080p without replacing the desktop.
- End Xfce through the UI and confirm GNOME's service PID remains unchanged.

Evidence: `artifacts/managed-broker-proof/<host>/gnome-input.png`,
`xfce-input.png`, `inventory.png`, `gnome-survives.png`, `report.json`.
These generated artifacts are git-ignored. Browser captures are 1280×900;
desktop screenshots were inspected at 1080p rather than ingesting 4K images.

## Issues found during implementation

The inherited Xfce autostart suppression also masked GNOME SettingsDaemon.
GNOME now retains its settings components. Minigpu's Mutter inferred a Wayland
launch despite the private X11 display; its instance-local Shell desktop entry
now explicitly passes `--x11`. No system desktop entries were modified.

Minigpu also inherited `GNOME_SETUP_DISPLAY` from the physical user manager.
This caused its managed XSettings component to miss registration, producing a
delayed session failure. That variable and other physical session identifiers
are now removed before launching the managed desktop. Immediate video readiness
was insufficient to catch the delayed failure; the concurrency test holds both
GNOME desktops beyond the 90-second component registration deadline.

An earlier concurrency run observed clean logouts while Andrew was testing;
Andrew confirmed he was logging out desktops. Those interrupted runs are not
evidence of either lifecycle success or a daemon failure.

Ctrl+Alt+T did not launch a terminal in the initial GNOME test. Activities search
is the verified launch path. Browser Meta/AltGraph remains explicitly unsupported;
the test does not hide this by injecting a process over SSH.

## Regression checks

`managed-broker-concurrent.mjs` passed on both hosts after separating Andrew's
interactive testing from the automated run: two GNOME desktops remained alive,
duplicate create returned the same ID, normal private-bus GNOME logout retired
only the selected desktop, the sibling PID stayed unchanged, and no replacement
was created. Survivors at handoff: Minigpu `9f43ef60693c404ba0b9008c158c02ec`,
Spark `8614c33554db4409b973435d1d06b3c8`.

GNOME and Xfce are marked validated for these two pilot hosts only. This is
functional pilot acceptance, not a long-duration reliability claim.

- Python managed adapter/broker tests: 9 passed.
- Frontend tests: 73 passed.
- Draft local/protocol contract tests: 106 passed.
- Python compilation and frontend production build checked.

## Limits

This is a trusted-LAN, fixed-localuser pilot, not authenticated fleet brokering.
Private Xorg GNOME is not physical Wayland capture. Same-account instances share
home/filesystem authority, and some applications have separate profile or
singleton constraints. Native terminal protocol and arbitrary installed desktop
adapters are not implemented. No host/GDM restarts were used for this work;
Minigpu physical Wayland session 214 was verified active after managed tests.
