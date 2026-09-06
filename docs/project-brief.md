# Project brief

Andrew Morgan wants a Linux-first console-sharing and terminal system with an original web UI, Keycloak login and eventual daemon enrollment across attic, office, minigpu and Spark. Existing console sharing is sufficient; visible local output/input is acceptable. M1 does not implement Microsoft's RDP protocol or RDS multi-user sessions.

## M1

One installation on one pilot host: Rust worker plus .NET 10 broker serving a React UI and a reusable browser session package. The controlling browser runs on a different machine. Prove 4K quality, input, PTY and auth before separating deployment.

Spark is the proposed pilot because it currently has an X11 session and ARM64 is useful early coverage. Its encoder/4K capability is unverified. Hardware or production changes are not authorized by this bootstrap.

## Non-negotiables

- Good native 3840×2160 text and motion; measure fidelity, frame pacing and latency separately.
- Original DonkeyWork appearance and implementation; no Guacamole or RustDesk web app.
- Rust for device/media/PTY work; .NET for broker/auth; TypeScript and selective WASM for browser integration.
- A real PTY, working input, permissioned clipboard and reliable reconnect.
- Standard authentication/crypto and explicit privilege/session boundaries.
- Independent versioned protocol; RustDesk compatibility is not required.
- Open-source intent, outbound licence to be selected; no verbatim upstream copying.

## Later stages

M2 moves the broker to attic and adds enrollment, persistent device identity, online status, revocation and fleet UI. M3 exposes the same sessions through MCP with human takeover. M4 provisions computer-use desktops through DonkeyWork Sandbox, including Vault-backed credential paste.

Cloudflare is a preference for web/API traffic, not a restriction on media transport. Static public IPs and UniFi VPN are available options. Desired eventual hostname: rd.donkeywork.dev.

The fuller brief and source-research record are in Obsidian Me, Personal/DonkeyWork Desktop. Repo contracts are authoritative for implementation. Changes to agreed scope should update both records rather than allowing them to diverge.

## Hard requirement added 2026-09-06

The UI must change the actual remote desktop resolution while its session remains active. This is not browser scaling or encoder downsampling. M1 must demonstrate 3840×2160 -> 1920×1080 -> 3840×2160 without reconnecting the desktop or disrupting the terminal. Unsupported resize on the selected pilot blocks M1 completion.
