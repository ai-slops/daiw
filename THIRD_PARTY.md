# Third-party software and fixtures

No audio from third-party songs, commercial sessions, or plugin binaries is included. The shared-chorus fixture is synthetic, authored for this repository, and distributed under this repository's MIT license.

| Component | Role | License |
|---|---|---|
| React / React DOM | Client interface | MIT |
| Vite | Client build | MIT |
| TypeScript | Type checking | Apache-2.0 |
| Lucide | Interface icons | ISC |
| ky | Client HTTP | MIT |
| Zod | Boundary schemas | MIT |
| Rust / standard library | Domain and audio runtime | MIT / Apache-2.0 |
| serde / serde_json | Typed wire format | MIT / Apache-2.0 |
| thiserror | Domain error types | MIT / Apache-2.0 |
| BLAKE3 | Canonical fingerprints | CC0-1.0 / Apache-2.0 / Apache-2.0 with LLVM exception |
| sha2 0.10.9 | Binary PCM payload/file integrity | MIT / Apache-2.0 |
| SQLite via Node.js | Transactional persistence | Public domain; Node.js MIT |
| Symphonia | WAV/AIFF/FLAC decoding | MPL-2.0 |
| CPAL | Optional native audio output | Apache-2.0 |
| Electron | Sandboxed desktop shell | MIT, with bundled Chromium/Node third-party notices |
| UUID | Stable newly created entity IDs | Apache-2.0 / MIT |

The exact dependency graph is pinned in `package-lock.json` and `Cargo.lock`. Transitive licenses must be checked from these lockfiles before binary redistribution. `npm sbom --sbom-format cyclonedx` produces the JavaScript SBOM. `cargo metadata --format-version 1` includes the Rust package/license inventory. Native codec, audio backend, and Electron dependencies are documented alongside their implementation; their distribution obligations are not waived by this table.

Symphonia's MPL-2.0 obligations apply to its covered source files. Preserve its notices and make the corresponding covered source available when distributing binaries. We do not copy or modify codec implementations in this repository. Electron distributions must retain Electron and Chromium's bundled license notices.

Existing `.dim` files are environment infrastructure preserved as supplied, not newly relicensed source.
