# Local agent CLI — first component slice

Original Rust client for `dwdesktop.local` 0.2.0. It is a standalone Cargo package
inside this repository; canonical schemas are embedded at build time from
`contracts/schemas/`. No RustDesk source is included or adapted.

Implemented: `describe`, `open`, `close`, `screenshot`, `click`, `key`, `text`,
`resize`, and one-shot `control`. `term` deliberately fails with
`not_implemented`; interactive PTY and persistent scripted-control renewal are
follow-up work. This component does not complete M1a or prove any physical-host
capture, input or mode change.

## Build and checks

```sh
cargo build --locked --manifest-path cli/Cargo.toml
cargo test --locked --manifest-path cli/Cargo.toml
cargo clippy --locked --manifest-path cli/Cargo.toml --all-targets -- -D warnings
```

Linux is required for kernel peer credentials. JSON Schema validation runs
offline with local common definitions; no schema URLs are fetched at runtime.
The source baseline is `1b27a7e14c91e9ed20f55639c84b17514fb78d06`.

## Example workflow

These are invocation examples, not permission to start services or change a lab
display. Replace the socket and service UID with the administrator's configured
local endpoint. Prefix options must appear before the command.

```sh
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 describe
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 open --context ./session.json
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 screenshot --context ./session.json --display display-0 --output ./screen-1.png --snapshot ./snapshot-1.json
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 click --context ./session.json --snapshot ./snapshot-1.json --x 200 --y 150
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 screenshot --context ./session.json --display display-0 --output ./screen-2.png --snapshot ./snapshot-2.json
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 key --context ./session.json --snapshot ./snapshot-2.json --usage 40
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 resize --context ./session.json --snapshot ./snapshot-2.json --width 1920 --height 1080
cli/target/debug/dwdesktop --socket /run/dwdesktop/agent.sock --server-uid 1000 close --context ./session.json
```

`key --usage 40` sends HID Enter down/up under one lease. `text` reads bounded
UTF-8 from stdin, never a password/text argument. It sends exactly what it reads,
including any final newline. It does not print text. Full key chords and long
script transactions are not implemented in this first slice.

Take a fresh screenshot after any resize and use that new metadata for input.
The server enforces snapshot retention and current topology; a local JSON file
is not proof that an application target has stayed still. No input is retried
after ambiguous delivery. Each modifying command acquires/releases a lease on
one connection; `control` checks acquisition and immediately releases, while
`control --reset` also releases held input. It cannot export a usable lease into
another process. Operations that exceed the server's lease fail closed; no
background renewal loop is implemented yet.

## Trust and artifact handling

- The configured server UID is checked against Linux `SO_PEERCRED`, not a JSON
  claim. The socket must be owned by that UID with no world permissions, and its
  immediate directory owned by the same UID without group/world write access.
- The daemon independently authorizes the CLI UID and fixed account/profile.
  Context stores endpoint, expected UID, session ID and epoch only; it is
  user-selected metadata, not bearer authentication. Session retention remains
  the daemon's 120-second maximum after disconnect.
- Context, snapshot and PNG outputs are created exclusively at mode 0600, with
  no following of destination symlinks or overwriting of existing files.
  Failed operations may leave empty/partial reserved artifacts; delete those
  explicitly before retrying with new requests. Invalid metadata cannot drive
  input. Successful `close` leaves the context file as a record.
- Header length/schema/correlation/epoch, 64 MiB PNG payload and decoded raster
  bounds are checked. PNG CRC/decompression uses the maintained `png` library.
  This first client accepts non-interlaced RGBA8 PNG with IHDR/IDAT/IEND and
  optional sRGB/gAMA/cHRM only. Other ancillary chunks, APNG and interlacing
  fail explicitly. Coordinate mapping never silently scales.
- Structured results go to stdout; safe error codes/static messages to stderr,
  with nonzero exit status. A rejected resize prints its actual-state result
  and exits nonzero. Daemon free-form messages and invalid argument values are
  suppressed to avoid reflecting secrets. No PTY, input or image bytes in logs.

## Verification and limits

Tests run the actual CLI against isolated Unix fixture servers, verify canonical
message shapes, observe/click/observe ordering, key sequencing, private files,
correlation rejection, UID/socket checks and sanitized failures. A synthetic 4K
PNG exercises exact raster decoding, bounds, malformed/animated PNG rejection.
These are fixture/component checks, not a real desktop benchmark or security
audit. Remote CLI authentication, broker/browser streams and PTY are absent.

Direct dependencies: clap (MIT/Apache-2.0), jsonschema (MIT), nix
(MIT), png (MIT/Apache-2.0), serde/serde_json (MIT/Apache-2.0), uuid
(MIT/Apache-2.0); tempfile (MIT/Apache-2.0) is test-only. Cargo.lock pins the
resolved transitive dependencies. The repository's outbound license remains a
separate decision; no upstream license boilerplate has been copied.
