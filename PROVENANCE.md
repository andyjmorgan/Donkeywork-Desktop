# Provenance and licensing

This is an original implementation inspired by remote-desktop engineering, including study of RustDesk. The owner intends to publish open source and does not intend to sell the project.

## Current boundary

No RustDesk source, assets, copied UI or verbatim implementation is included in this bootstrap. Its public repository was inspected at commit 692113c with hbb_common at 3d6fb2c397f2a9a717440f7e29afed5ab5f5dc03. This process must not be described as clean-room.

RustDesk is AGPL-3.0. Public, free or noncommercial development is not a substitute for complying with source/licence obligations if code is reused or adapted. A rewritten UI or process boundary alone does not resolve that question.

**Outbound project licence is not selected yet.** The public repository is not yet a claim of a particular open-source grant. Select a licence before distributing implementation releases; do not infer permission to use MIT/Apache/AGPL simply from this brief.

## Dependency register

| Dependency | Purpose | Upstream licence |
|---|---|---|
| ajv | Development-only JSON Schema validation | MIT |
| ajv-formats | Development-only format validation | MIT |

Exact versions/integrity are pinned in package-lock.json. Preserve upstream notices as required. Add runtime dependencies here with their purpose, source URL, licence and any redistribution obligations before adoption.

For source imports/adaptations, record upstream URL, exact revision, files, changes and licence; obtain an explicit project decision before RustDesk-derived implementation. Behavioural benchmarks, conceptual ideas and copied code are different forms of influence and must be described accurately.
