# Project brief

Andrew Morgan wants a Linux-first console-sharing and terminal system with an original web UI, Keycloak login and eventual daemon enrollment across attic, office, minigpu and Spark. Existing console sharing is sufficient; visible local output/input is acceptable. M1 does not implement Microsoft's RDP protocol or RDS multi-user sessions.

## M1a — agentic engine first

One X11 pilot, Rust daemon and agent-facing CLI over a dedicated Unix socket. Required: display enumeration, fresh native-4K PNG screenshots with machine-readable metadata, coordinate clicks, keyboard input, actual 4K -> 1080p -> 4K mode changes preserving session and PTY, and real terminal access. Explicit allowed peer UIDs map to permissions and fixed OS profiles; no implicit root or any-local-user access. The CLI can be invoked through an existing separately authorized shell connection, but M1a adds no remote listener. This intentionally scopes M1a to local authentication; remote OAuth is not an M1a completion requirement.

## M1b — live browser desktop

Add the co-located .NET 10 broker, Keycloak and React/browser session package. Prove native 4K browser fidelity, streaming latency and live UI resolution changes from another machine. Screenshots alone cannot validate this milestone. Remote CLI API/auth design belongs here too; device flow supports operator login, while unattended credentials require a separate least-privilege decision.

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

Actual remote desktop resolution must change during an active session: via CLI in M1a and via web UI in M1b. This is not browser scaling or encoder downsampling. Both demonstrate 3840×2160 -> 1920×1080 -> 3840×2160 without reconnecting the desktop or disrupting the terminal. Unsupported resize on the selected pilot blocks completion.
