# Verification ledger

This file records evidence, not aspirations. The supplied design remains the product target; the current repository must not be described as having passed every stage-1 acceptance condition.

## Latest follow-up: incremental instance patterns (2026-10-09)

- **242 Rust**, **56 host/API**, **71 client tests** passed. Worker rebuild, Rust formatting, all-feature workspace/all-target Clippy and strict TypeScript/Vite build passed. Changed Rust/MJS diagnostics were clean; upstream Zod annotation warnings remain non-blocking.
- New tests cover exact post-modifier work-trace replay, disabled/cold/warm equivalence, proposal isolation, stale guards, downstream placement/crop/track changes, slot invalidation, provenance and memory/admission bounds. Existing source-cache regressions remain green after extraction of shared LRU storage.
- Independent review caught an error-precedence change and unnecessary deep-copy/admission work. The previous operation-validation → track → event-budget → pattern sequence was restored and regression-tested. Cold resolution moves owned payloads; oversized entries are checked before retained copying. A final lifetime-only adjustment drops the unused composed vector before cloning a cached pattern, followed by targeted tests and diagnostics.
- Live HTTP results for a modified fixture: cold instance resolution **6 misses / 0 hits / 4 operation-work units**; warm **0 misses / 6 hits / 0 units**. Moving the clip retained all six hits with correctly shifted notes. Local transposition yielded **1 miss / 5 hits / 12 units**, with updated target pitches.
- Diagnostic reads left both counter groups intact, and agent diagnostics returned 403. Restart cleared both caches. After removing render files, the edited snapshot and regenerated **13,165,760-byte WAV** remained identical; SHA-256 `c17921a958fd26925701e0d434242a3b697bf5f62351dbdf85b37870c065415d`. The disposable store was removed.
- UI unchanged; no browser/visual run was needed. No elapsed-time speedup, full DSP incremental compilation, streaming, physical-device or Windows qualification is claimed.

## Incremental source composition (2026-10-09)

- **230 Rust**, **52 host/API**, **71 client tests** passed. Rust fmt, all-feature workspace/all-target Clippy, worker rebuild and strict TypeScript/Vite production build passed. Changed Rust/TypeScript/MJS diagnostics were clean. Existing upstream Zod annotation warnings remain non-blocking.
- Full disabled/cold/warm response comparisons cover snapshot, explain, note selection, dry-run and stale guards. Key tests cover same-revision content changes, descendant provenance, excluded metadata, reordered children versus flat note order and project isolation. Resource tests cover aggregate bounds, LRU eviction, oversized skips and exact work-error replay.
- Independent review caught replay divergence after an already-exceeded work meter, incomplete backing-storage accounting and mixed-size admission dropping a fitting candidate after eviction. Each was fixed with a failing-then-passing regression. Retention now explicitly counts owned capacities, not allocator metadata or process RSS.
- Independent live HTTP sequence: cold snapshot **4 misses / 2 hits / 20 expanded units**; warm snapshot **0 misses / 6 hits / 0 expanded units**, with identical snapshot. A child edit produced **2 misses / 4 hits / 13 expanded units** and changed audible output. Residency then measured 6 entries, 27 notes, 8 path entries and 5,867 accounted bytes.
- The stats query preserved state and its own last-request counters; agents received 403. Service restart cleared cache residency. After deleting rebuildable render files, the regenerated WAV exactly matched the prior edited WAV: **13,165,760 bytes**, SHA-256 `015ad9ff966931ae2e180618e61c290f1b7308985379d0509ac7c81a4462ac2c`. The manual driver's disposable store was removed.
- No UI changed, so browser/visual checks were not repeated. No elapsed-time speedup, long-session benchmark, physical-device or Windows certification is claimed. This is source-pattern reuse only; operations, effective events and DSP are still recomputed.

## Nested note composition (2026-10-09)

- Rust workspace tests **208 passed**, all-feature Clippy and formatting passed; host/API tests **50 passed**, client tests **68 passed**, strict TypeScript/Vite build passed. Existing upstream Zod annotation warnings remain non-blocking.
- Regression coverage includes child/grandchild looping and cropping, DAG cycles/depth/shared paths, work/event budgets, stable nested IDs, flat native hash compatibility, source/local selection, one-occurrence patches, descendant binding/proposal staleness, and before/after dependency impact.
- Independent code review led to fixes for missing closure guards, legacy revision overflow, stale proposal previews, request-wide work accounting, exact emitted-event counting and a stale flat AddNotes proposal collision. The collision regression rejects both stale preview and acceptance without changing HEAD.
- `tests/nested-composition.test.mjs` drives real HTTP/Rust/SQLite: child source changes propagate to untouched notes and peer instances while a locally edited occurrence remains independent; native export/import and service restart preserve state; render produces nonzero PCM; cycles fail without HEAD change; removing a patched child is reported unready by dry-run.
- A separate manual HTTP driver observed two scoped nested notes with root/leaf provenance, hashed nested effective IDs, agent apply rejection (403), cycle rejection, required native feature and a non-silent **13,165,760-byte WAV**. Its disposable data store was removed.
- Real Chromium QA covered placement add/edit/navigation, own notes versus read-only ghosts, invalid-input draft retention, failed-save retry/cancel, and project-switch path reset. Lead independently drove blank-length rejection, corrected placement save, two ghosts and child source-only navigation against a freshly started isolated server. Independent responsive and error-state visual reviews passed. Review captures were ephemeral and some were cleaned afterward; this ledger records observed checks rather than promising retained artifacts.
- Changed Rust/TypeScript diagnostics passed. CSS LSP remained unavailable; build and real-browser checks supplied CSS validation. No physical audio-device, Windows, performance-scale or full composition-graph qualification is claimed.

## Timing maps (2026-10-04)

- **176 Rust**, **49 host/API**, **57 client tests** passed; strict TypeScript/Vite build, Rust fmt and Clippy workspace/all-targets/all-features with warnings denied passed. Existing Zod annotation warnings remain non-blocking.
- Regression coverage includes step/ramp integration and inversion, invalid public conversion inputs, exact meter boundaries, cross-runtime large-tick acceptance, native feature/hash compatibility, absolute mapped frame placement, and scalar waveform compatibility.
- Integration review found and fixed JS/Rust tick tolerance and meter arithmetic divergence, client finite-pitch schema mismatch, truncated imported source tails, and looped seek scheduling mismatch. The 48,019-frame import regression verifies the final source frame survives rendering.
- An independent live HTTP driver dry-ran as agent, rejected agent apply, applied both maps as human, rejected an off-bar meter change, exported both required native features, imported, restarted the service and reopened identical state. Rendered WAV contained exactly **2,075,699 frames / 8,302,840 bytes**, matching independently integrated expected duration, with nonzero audio.
- Real Chromium QA exercised map editing, active 5/4 position `003 / 01 / 000`, four-bar looping, playback start/pause and constant resets. Lead independently exercised cancel/reopen and actual tempo/meter saves. Lifecycle QA covered disabled busy controls, committed-revision confirmation and inline stale-error presentation; its error-response case used browser interception. Independent visual reviews passed responsive and lifecycle captures.
- Evidence is in `/tmp/opencode/daiw-timing-evidence/` and `/tmp/opencode/daiw-timing-final/` (ephemeral container artifacts). Rust/TS diagnostics passed; CSS LSP remains unavailable, with build/browser checks used instead. No physical-device, Windows or long-session certification is claimed.

## Native exchange and bounded selection (2026-10-03)

- **159 Rust tests**, **41 host/storage/HTTP tests**, and **39 client tests** passed. Rust fmt, Clippy workspace/all targets with warnings denied, rebuilt workers and TypeScript/Vite production build passed. Upstream Zod annotation warnings remain non-blocking.
- Native tests cover legacy/envelope import, unknown versions/features/semantic fields, canonical hash integrity, signed zero, portable integers and finite-float round trips. Selector tests cover deterministic bounds, onset membership, source/local scope, project/revision binding and human-only apply.
- Independent review reproduced a no-op source transform advancing the definition revision. A failing regression now passes: empty and unchanged selected transforms return `InvalidParams` before edit mutation.
- Independent live HTTP driver queried one note, dry-ran/applied local transposition, rejected a stale handle with 409, rejected tampered content and a future version, imported under a new identity, reopened an identical state after service restart, and rendered a non-silent **13,165,760-byte WAV**. The disposable database was removed.
- Browser file import was exercised through real Chromium in Docker: invalid JSON displayed an error; native JSON opened a new project and closed the dialog. Worker QA additionally covered unsupported versions. Independent visual review found mobile error wrapping and duplicate dimmed toast issues; both were fixed. Fresh normal/error captures at 375, 768 and 1280 pixels passed independent review. Evidence: `.omo/evidence/import-ui/review-*.png`.
- Rust and TypeScript LSP diagnostics passed on the changed selector/import modules and UI. CSS LSP was unavailable; production CSS compilation and browser captures supplied validation instead. Local Chrome dependencies could not be installed within download timeouts, so browser verification used the Playwright Docker image.
- Contract clarifications: hashes cover typed/default-normalized canonical content, not raw document bytes; instance edits own whole note records, not individual fields. Review also identified the pre-existing unauthenticated decoded-cache payload limitation, recorded in [native-exchange.md](native-exchange.md), not fixed in this batch.
- Physical audio devices, Windows desktop, long-session performance, full media archives and migration qualification were not tested. No full stage-0/1 completion is claimed.

## Previous follow-up: renderer numeric safety

- **139 Rust tests** passed, default features (17 new numeric/rounding/allocation regressions).
- **36 host/storage/HTTP tests** passed (3 new finite frequency/float-PCM failure and recovery cases).
- Rust fmt, Clippy workspace/all targets with warnings denied and rebuilt core/render workers passed. No callback changes were made; the existing allocation/deallocation and transport tests stayed green.
- Numeric regressions cover finite pitch overflow/underflow, finite-frequency phase overflow, linear-gain overflow, f32 track/master overflow, non-finite direct WAV input, preserved finite clipping, ignored/empty paths, half-frame rounding, scratch reset, exact frame cap and typed reservation failure.
- HTTP regressions verify valid finite snapshots followed by 422 `AudioError`, no failed WAV files or `renders` rows, preserved exported state and undo recovery. Extreme finite float WAV import exercises the PCM region path rather than only synthesized notes.
- Focused Oracle design and final implementation checks found no blocking deviations within the stated numeric/render-budget scope.
- Independent HTTP QA confirmed frequency failure before and after service restart, unchanged cache/HEAD, undo recovery to non-silent 13,165,760-byte WAVs, a successful exact 180-second/48-kHz render of 34,560,044 bytes and a host-side 413 `ResourceLimit` above that duration.

Before/after WAV SHA-256 comparison on the current Linux build target:

| Fixture | Bytes | SHA-256, identical before and after |
|---|---:|---|
| Fractional synth | 186604 | `39ad714d718aae04192d302d9a570d2ab874f33eb7154735dc37f08e158d3a84` |
| Cropped synth | 93324 | `5743c3867c6bd672baa782bf6f75551215d98bedb6750a960b5422c2d5f4167f` |
| Pitch-independent kick | 186604 | `00cf63b64a0b5e3924e8a199e0bb66ea4ead2d14bdd6b1464fde2e9c1911a4c0` |
| Resampled stereo | 186604 | `2b68f4240ca16e0a3611353c97074e87a5ba2db97b518d149c3dca80f60af3a1` |

These fixtures use 48 kHz stereo, tempo 123.5, track gain -3.25 dB, pan 0.3 and cutoff 4321 Hz. The normal tone is pitch 60.25/velocity 0.8, start 0.25 beat, duration 1 beat in a 2-beat snapshot. The crop case uses a 1-beat snapshot with nominal 3-beat note duration. The kick case uses the normal timing with finite maximum pitch. Stereo PCM uses 441 stereo frames at 44.1 kHz (`samples[i] = sin(i * 0.12) * 0.5`), region start 0.25/duration 0.5 beat, gain 3 dB and source offset 0.001 s. These are current-target differential evidence, not cross-platform libm bit-equivalence promises.

Manual QA used disposable stores and actual HTTP/render workers; artifacts were removed. Client/UI files were unchanged; their prior tests/build and browser checks were not rerun. No native-feature, physical-device or Windows qualification was performed. Rust LSP is unavailable, so compiler/Clippy supplied static checks. The output frame cap is not a total-process-memory/CPU guarantee; assets, input JSON and host buffers remain additional costs. See [audio boundaries](audio.md#numeric-and-output-memory-boundaries).

## Previous follow-up: final effective-note validation

- **122 Rust tests** passed, default features, including 14 new core/renderer regressions.
- **33 host/storage/HTTP tests** passed, including duplicate source-ID and finite-operand overflow cases across dry-run, explanation, snapshot, render, export/import and undo.
- Rust fmt, Clippy workspace/all-targets with warnings denied, and rebuilt core/render workers passed.
- Core checks emitted per-instance notes after repetition/crop and the aggregated snapshot. Source/modifier collisions, slot replacement collisions, cross-instance encoded-ID collisions, final NaN/Infinity/velocity/timing violations, transient repair, proposal isolation and fractional pitch are covered. Standalone audio input rejects duplicate IDs and retains finite-value checks.
- Independent HTTP usage exercised duplicate and overflow cases, each returning 422 `InvalidProject` for snapshot/render without replacing state. Both unresolved projects survived service/SQLite restart unchanged; undo recovered a non-silent 13,165,760-byte WAV in each case.

The driver used disposable project data and actual HTTP/core/render workers; temporary artifacts were removed. Client/UI files were unchanged, so no fresh client-test/Vite/browser claim is made. No native-feature, Windows or physical-device qualification was run. Rust LSP is unavailable; compiler and Clippy supplied static checks. That follow-up validated note data; renderer arithmetic was addressed separately in the latest entry above.

## Previous follow-up: explicit modifier order and access algebra, 2026-09-22

- **108 Rust tests** passed (19 new graph/algebra/lifecycle regressions), default features.
- **31 host/storage/HTTP tests** passed, including dry-run refusal of undeclared order, cyclic/unauthorized/stale apply rejection, audible-note order change and native export/import preservation.
- **31 client tests** passed, including absent versus explicit-empty metadata, required edges and typed derived-access report parsing.
- Rust fmt/Clippy (workspace/all targets, warnings denied), rebuilt workers, strict TypeScript and production Vite build passed. Existing upstream Zod annotation warnings remain non-blocking.
- Oracle review found an AddNotes hard-conflict coverage gap within single modifiers and against local overrides. Two failing regressions reproduced it; an explicit-mode prepass over the full operation stream fixed it, and the final suite passed. No claim of a separate post-fix Oracle approval is made.
- Independent HTTP QA observed legacy added-note pitch 60 versus explicit add-before-transpose pitch 72, non-mutating `ModifierOrderRequired` dry-run, HTTP 422 cyclic-graph rejection without HEAD replacement, and distinct non-silent 13,165,760-byte WAVs before/after ordering. Service/SQLite restart preserved the graph and produced a byte-identical post-change WAV.

The driver used disposable project data and actual HTTP/core/render workers; artifacts were removed. LSPs are unavailable; compiler/Clippy/TypeScript supplied static checks. No browser/visual, native-feature, physical-device or Windows qualification was run. The [ordering contract](contract.md#explicit-modifier-dependency-ordering) documents opt-in legacy compatibility, graph limits, conservative resources, floating-point/provenance caveats and unsupported downgrade.

## Previous follow-up: persisted modifier slot bindings, 2026-09-22

- **89 Rust tests** passed, default features, including 12 new attachment/rebase/legacy/validation tests.
- **30 host/storage/HTTP tests** passed, including slot binding export/import, dry-run, stale revision and agent rejection, explicit rebase, and effective replacement output.
- **27 client tests** passed, including metadata preservation, nested-shape rejection, legacy absence and structured binding diagnostics.
- Rust fmt, Clippy workspace/all-targets with warnings denied, worker rebuild, strict TypeScript and Vite production build passed. Upstream Zod annotation warnings remain non-blocking.
- Focused Oracle design and final implementation checks found no blocking deviation from the Strong-only canonical-slot contract.
- Independent HTTP driver observed stale binding persistence across restart, render rejection with 422 `RebaseConflict`, non-mutating rebase preview, stale apply rejection with 409, metadata-only revision 0 → 1 rebase, a non-silent 13,165,760-byte WAV, and preserved binding/replacement pitch 80 after a second database/service restart.

QA used a disposable SQLite store and actual HTTP/Rust workers; temporary data was removed. LSPs are unavailable; compiler/Clippy/TypeScript supplied static checks. No browser/visual, native-feature, Windows or physical-device qualification was run. Old-core downgrade is unsupported because old readers can drop binding metadata. See [the binding contract](contract.md#persisted-strong-slot-modifier-bindings).

## Previous follow-up: scoped dry-run reports, 2026-09-22

- **77 Rust tests**, default features, passed (11 new dry-run/impact regressions).
- **29 host/storage/HTTP tests** passed, including an authenticated agent request that reports local target failure and shared-source impact without changing exported state.
- **23 client tests** passed, including optional-report preservation, legacy compatibility and rejection of unknown resolution states.
- Rust formatting, Clippy workspace/all-targets with warnings denied, worker rebuild, strict TypeScript and Vite production build passed. Existing upstream Zod annotation warnings remain non-blocking.
- A focused Oracle review found no blockers for the instance-centric/first-loop-note scope; it does not certify full render readiness or generic entity impact.
- Independent HTTP usage: candidate source deletion reported missing `note-chorus-root` in chorus A and all three chorus dependency ranges (24–32, 56–64, 120–128); corrective dry-run reported conflict → clean; a healthy neighbor remained ready; stale-base dry-run returned `StaleRevision` without candidate reports. Exported canonical state was unchanged by all dry-runs, and the original snapshot remained usable before applying the deliberately conflicting test edit.

Manual QA used an ephemeral loopback host, actual Rust workers, and a disposable SQLite store; artifacts were removed. No browser/visual, native-feature, physical-device or Windows qualification was run in this follow-up. LSPs are unavailable; compiler/Clippy/TypeScript served as static gates. [The contract](contract.md#scoped-dry-run-impact-and-note-resolution-reports) explicitly separates structural `executable` from scoped `noteResolution.ready`.

## Previous follow-up audit: local note edits, 2026-09-22

See [progress.md](progress.md) for the original-plan coverage matrix, completed note-identity work, and next priorities. Fresh verification: **66 Rust tests (default features), 28 host/storage/HTTP tests, 20 client tests**, Rust formatting, Clippy all-targets/default-features with warnings denied, rebuilt workers, and strict TypeScript/Vite build passed. Twelve Rust regressions exercise note patches and their boundaries; client tests cover schema contexts. The HTTP integration test covers source propagation, native project export/import, and blocked conflicting render.

An independent disposable HTTP driver observed local pitches `[49,77,68,66]` versus peer `[61,77,68,66]`, a non-silent 13,165,760-byte WAV, HTTP 422 `RebaseConflict` for a removed local target, successful healthy-peer explanation, and undo plus SQLite close/reopen preserving the edits. This is rendered-signal verification, not physical speaker or musical-quality certification.

LSPs remain unavailable; compiler/Clippy/TypeScript are the substitute gates. Physical devices, Windows, native-feature builds, browser/visual QA, package audits and license inventory were not rerun in this follow-up. Existing upstream Zod annotation warnings remain non-blocking.

## Historical vertical-slice implementation results

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

1. **Complete composition graph:** nested components, general FlowGroup/AbsoluteGroup placement, full exposed-parameter bindings, arbitrary stable selector ASTs and complete rebase UI remain open. Strong canonical slot bindings, an opt-in current-operation modifier DAG, final emitted-note checks and bounded renderer numeric safeguards now exist, but do not complete the general composition graph.
2. **Streaming and incremental compilation:** the host uses bounded decoded PCM and offline renders. It does not yet provide full-session disk streaming/read-ahead or the design's incremental resolver/graph compiler.
3. **Unified native live editor output:** the CPAL adapter is independently implemented; the desktop client currently auditions canonical renders through Chromium. A native audio-control RPC path and device-selection UI remain to be integrated and qualified.
4. **Full signal/device model:** the current built-in strips are not the complete typed signal graph, TrackFacade/Part/Lane split, general routing, automation, or device-capability model.
5. **Complete editing surface:** the piano roll supports basic note insertion/removal and explicit source/local scope, not every move/split/trim/stretch/expression workflow in the design. New local edits now use normalized stable-ID `patchNotes`, preserving unrelated source changes; full-record local updates still override all properties of that note, and legacy `ReplaceSlot` remains a full-pattern replacement. Per-field merging, full rebase/impact UI, detailed audio-region editing, and full keyboard/screen-reader note authoring remain open.
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
