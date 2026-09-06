# Dependency provenance

Code in this component is original implementation against public X11/RandR/XTEST/XKB APIs and the repository's approved contracts. RustDesk was an earlier research reference and is neither a dependency nor a source of adapted code. The project's outbound licence remains an integrator/user decision; no third-party project licence is applied to our code here.

Direct dependencies are Cargo packages fetched from crates.io and fixed by this directory's Cargo.lock:

| Package | Locked version | Declared licence | Purpose |
|---|---|---|---|
| x11rb | 0.13.2 | MIT OR Apache-2.0 | X11/RandR/XTEST/XKB client |
| x11rb-protocol | 0.13.2 | MIT OR Apache-2.0 | Display parsing and Xauthority handling |
| png | 0.18.1 | MIT OR Apache-2.0 | Lossless RGBA PNG encoding/validation tests |
| rustix | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Deadline-bounded file descriptor polling |

Transitive declarations from `cargo metadata` on 2026-09-06:

| Package/version | Declared licence |
|---|---|
| adler2 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| bitflags 2.13.1; cfg-if 1.0.4; crc32fast 1.5.1; errno 0.3.14; fdeflate 0.3.7; flate2 1.1.10; libc 0.2.189; windows-link 0.2.1; windows-sys 0.61.2 | MIT OR Apache-2.0 |
| gethostname 1.1.0 | Apache-2.0 |
| linux-raw-sys 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| miniz_oxide 0.8.9 and 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| simd-adler32 0.3.10 | MIT |
| zlib-rs 0.6.7 | Zlib |

Metadata can include optional or target-specific packages not linked in a given build. Distribution must preserve applicable upstream licence/notice files; this inventory records provenance and is not a completed packaging compliance assessment. No libraries are vendored here.
