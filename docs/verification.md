# Verification ledger

This file records evidence, not aspirations. The supplied design remains the product target; the current repository must not be described as having passed every stage-1 acceptance condition.

## Results from this implementation run

| Gate | Observed result |
|---|---|
| Rust workspace tests, native feature enabled | 55 passed |
| Rust Clippy, all targets/features, warnings denied | Passed |
| Rust formatting | Passed |
| Node host/storage/HTTP tests | 27 passed |
| TypeScript/browser logic tests | 18 passed |
| Strict TypeScript and production Vite build | Passed; upstream Zod annotation warnings are non-blocking |
| Chromium end-to-end workflow | Passed at 1440, 1280, 768, and 375px |
| Independent visual reviews | Both passed after layout/state corrections |
| Dependency license inventory | 272 JavaScript and 65 Rust packages checked |
| Production npm security audit | No reported vulnerabilities |

The browser driver exercises two consecutive instance-local note edits, clip switching, tempo edits, undo/redo, multiple proposals, A/B preparation and playback, partial approval, a keyboard mixer edit, measured master signal, project validation/history, modal interaction, and reopening after reload. Sixteen fresh screenshots cover Arrangement, Review, Mixer, and Projects at four viewport widths.

The native CLI was launched against a valid snapshot and reported the actual missing ALSA device. This demonstrates the hardware limitation, not successful native device output.

## Exercised implementation boundaries

- The same Rust command/resolution engine is used by the Node host and headless CLI.
- Linked definitions, local overrides, modifier conflict checks, exact 1/960-beat validation, origin metadata, and canonical fingerprints have regression coverage.
- Proposals are separate from main music until human approval. Host and core enforce agent restrictions; partial approval and dependency closure are tested.
- SQLite WAL snapshots, compare-and-swap, retry idempotency, history, import identity, corruption detection, and process-kill rollback have automated checks.
- Actual Rust WAV rendering, stereo source import, cache regeneration, and browser A/B preparation are exercised through HTTP.
- The native callback has allocation/deallocation instrumentation and a bounded SPSC snapshot-swap test.
- Browser audition uses canonical Rust-rendered audio. The master meter reads a real Web Audio analyser, not a simulated level.
- Host regression tests cover worker EPIPE/protocol failure, DNS rebinding, and Windows/POSIX static-path containment.

## Environment limitations

- This container has no physical Windows or Linux sound device. CPAL is compiled and its callback logic tested, but device opening, driver latency, xrun rates, and round-trip latency are not certified.
- Windows behavior is represented by a CI job and cross-platform path tests; a real Windows desktop session was not available in this turn.
- Browser playback was exercised in real headless Chromium. This observes transport, buffers, and measured signal, not a human assessment of musical quality or loudspeaker output.
- Native local Chromium/Electron libraries are absent in the DIM agent image; browser QA uses a separate Playwright container over CDP.
- LSP servers were unavailable. Rust compiler/Clippy and strict TypeScript compilation are the static verification gates instead; no claim of a clean LSP run is made.
- The configured Git remote hostname `dim-gitea` does not resolve from this agent container. Local commits can be created; successful remote push must not be claimed until connectivity is repaired.
- During final QA the DIM controller also became unavailable (`connect ENOENT /run/dim/controller/controller.sock`) and its public gateway returned HTTP 502. The application continued to return a healthy response locally. Final browser checks used a temporary direct test connection instead of the broken gateway.

## Outstanding original stage-0/1 requirements

The current implementation is intentionally explicit about these unfinished acceptance items:

1. **Complete composition graph:** nested components, a general FlowGroup/AbsoluteGroup model, full exposed-parameter bindings, stable selector ASTs on reusable modifier applications, modifier ordering DAGs, and a complete rebase UI are not all implemented. Current slots, proposal anchors, and rebase diagnostics are a smaller supported subset.
2. **Streaming and incremental compilation:** the host uses bounded decoded PCM and offline renders. It does not yet provide full-session disk streaming/read-ahead or the design's incremental resolver/graph compiler.
3. **Unified native live editor output:** the CPAL adapter is independently implemented; the desktop client currently auditions canonical renders through Chromium. A native audio-control RPC path and device-selection UI remain to be integrated and qualified.
4. **Full signal/device model:** the current built-in strips are not the complete typed signal graph, TrackFacade/Part/Lane split, general routing, automation, or device-capability model.
5. **Complete editing surface:** the piano roll supports basic note insertion/removal and explicit source/local scope, not every move/split/trim/stretch/expression workflow in the design. Local note editing currently uses a pattern-level `ReplaceSlot` override: later source-note changes are masked by that replacement, rather than merged per note. Precise note-level override merging, detailed audio-region editing, and full keyboard/screen-reader note authoring remain open.
6. **General AI provider integration:** the supplied natural-language client is a limited deterministic local adapter. A real external model provider and richer query/tool orchestration are not enabled or claimed. Explicit typed proposal operations remain available through the API.
7. **Production qualification:** long-session benchmarks, device buffer/sample-rate matrices, robust desktop packaging, exhaustive IME/high-DPI/accessibility testing, and power-loss testing on Windows remain release gates.

Recording, comping, external plugins, DAWproject exchange, collaboration, launcher, and ARA are separately planned in [roadmap.md](roadmap.md), not represented as implemented features.

## Reproduction commands

```sh
npm run core:build
npm run core:test
npm run test:server
npm run build
npm test
DAIW_URL=http://localhost:3000 node tests/studio.e2e.mjs
```

Native-enabled verification on Linux with ALSA development headers:

```sh
sh scripts/rust.sh build --workspace --all-features
sh scripts/rust.sh test --workspace --all-features
sh scripts/rust.sh clippy --workspace --all-targets --all-features -- -D warnings
sh scripts/rust.sh fmt --all -- --check
```

Build container and browser evidence are temporary tooling; generated binaries, caches, dependency directories, and screenshots are not canonical project data.

The browser driver creates a synthetic QA project in the target workspace. Use a test workspace for automation. `DAIW_CDP` can point to an existing browser endpoint when local Chromium cannot launch; `DAIW_EVIDENCE_DIR` controls screenshot output.
