# Original design progress audit

Audit date: **2026-09-22**. Target: the supplied 2026-09-01 v0.1 implementation design, indexed in [design-baseline.md](design-baseline.md). This is a code/test-based assessment, not a percentage estimate or release certification.

## Outcome

**2026-10-09 incremental-resolution follow-up:** the persistent worker now reuses dependency-keyed source compositions with bounded LRU retention and unchanged logical work limits. Snapshot/explain/query/dry-run/edit/proposal paths use explicit cache sessions; current guards and operations still run. Differential and live HTTP verification passed: **230 Rust / 52 host / 71 client tests**. See [incremental-resolution.md](incremental-resolution.md). This is not full incremental DSP compilation or disk streaming.

**2026-10-09 follow-up:** bounded nested note composition now supports stable child placements, source propagation, per-instance occurrence patches, path provenance, transitive impact and descendant-aware strong bindings/proposal guards. The Inspector edits placements and navigates sources without flattening; the root Source piano roll shows read-only nested ghosts. Full contracts and exclusions are in [composition.md](composition.md). This closes the initial nested-note subset, not the complete part/lane/flow model or incremental compiler.

**2026-10-04 follow-up:** step/linear tempo maps and independent meter maps now span core commands, native persistence, render timing, host duration limits, browser seek/loop, point-table editing and meter-aware transport/ruler. See [timing.md](timing.md). Validation: **176 Rust / 49 host / 57 client tests**, mapped export/import/restart/render and browser save/cancel/error checks. Remaining work includes nested composition, broader selector/application policies, incremental compilation/streaming and device qualification; this does not close the complete original roadmap.

**2026-10-03 follow-up:** versioned native JSON exchange with required-feature/integrity checks, legacy import, Projects file import UI, and bounded revision-bound note selection/transforms are implemented and verified. Latest counts: **159 Rust / 41 host / 39 client tests**. Real HTTP restart/render and responsive browser import evidence is recorded in [verification.md](verification.md). Contracts and remaining limits are in [native-exchange.md](native-exchange.md). The historical requirement map below predates this follow-up: full native archives/migrations and general selector/application policies remain open, but the native exchange envelope and initial SelectionHandle subset are no longer wholly absent.

**Stage 0 is partially implemented; stage 1 has an executable vertical slice but has not passed its full exit gate. Stages 2–4 remain deferred.** A functioning editor/demo is not proof that the original composition graph, persistence format, real-time compiler, or full query/review contract exists.

The full original document was absent. Existing `DESIGN.md` describes the visual system; the other docs summarize current code. The new baseline is an explicitly abridged English requirements digest covering the original 25 sections and appendices A–E, not a verbatim copy of the Korean source.

## Requirement-to-evidence map

| Original sections | Status and existing evidence | Still required |
|---|---|---|
| 1–3: product/scope/requirements | Defined in baseline, README, and roadmap; source/local and human-review flows exercised by `crates/core/tests/contract.rs`, `tests/service.test.mjs` | Complete P0 acceptance, including archive, richer time/editing/streaming and actual platform qualification |
| 4–5: architecture/state/identity | Rust command authority and canonical/effective separation in `engine.rs`, `resolver.rs`, `project.rs`; design decisions in `docs/architecture.md` | Full interconnected graphs, common revisioned entity headers, complete ADR set |
| 6: canonical model | Flat Definition/Instance/Modifier, tracks, assets/regions, proposals/history in `music.rs`, `project.rs`, `proposal_model.rs` | Nested placements, separate Part/Lane/TrackFacade, application entities/selectors, knowledge/constraint graph, full device/history/render models |
| 7: modifier composition | Current-operation read/write algebra, opt-in modifier dependency DAG, slot/parameter conflicts, local overrides, Strong slot bindings and explicit rebase | Arbitrary/Adaptive/Snapshot selectors, full application entities and general-domain conflict algebra, structural refactoring |
| 8: resolver/compiler | Deterministic effective notes with IDs/origins, looping/cropping, final emitted-note ID/scalar/timing checks, per-instance derived strips and expansion budget; resolver tests | Incremental dependency caches, nested/flow resolution, general signal schedule/PDC/stream plan and complete OriginPath |
| 9: time/transport | Exact 1/960-beat ticks; single tempo/meter representation; callback seek/loop tests | General rational domains, tempo/meter maps and inverse conversion, warp, broader count-in/clock/performance semantics |
| 10: audio | Rust WAV/sample rendering, built-in strips, callback allocation and swap/transport tests; real HTTP audio render | Full disk streaming, typed signal/device graph, live desktop/native-control integration, richer DSP and real Windows/Linux devices |
| 11: commands/queries/history | Typed commands, stale-base checks, dry-run with scoped note-resolution/instance-impact reports, snapshot/explain/history/hash, undo/redo and idempotency; host/core tests | Full SelectionHandle/capability/impact/semantic diff APIs, progressive queries, commit DAG/variants and rich dependency-aware hunks |
| 12: AI | Deterministic local phrase adapter plus explicit typed proposals, agent/main restrictions, partial acceptance | General model provider, scoped capability tokens beyond principal/method gating, retrieval/planning state machine, complete analyzer/review provenance |
| 13: UI | Existing arrangement, piano roll, inspector, mixer, projects and review surfaces; previous browser evidence in ledger | Full component/anchor/slot/routing/editor/accessibility workflows; no fresh browser/visual certification in this audit |
| 14: storage | SQLite WAL immutable snapshots/journal/CAS/idempotency/undo, content-addressed originals and rebuildable media caches; storage/crash/HTTP tests | Full native per-project manifest/schema/features format, immutable entity revision index, backup-first migrations, reachability GC, portable archives, recording recovery |
| 15: stack/repository | Rust core/audio, Node host, TS client, Electron shell, scripts and CI; THIRD_PARTY | Complete license/SBOM/contribution-policy gates and dedicated ADR/checklist closure; do not confuse current two crates with every proposed subsystem |
| 16–17: validation/security | Typed boundary parsing, references/duplicate IDs/resource limits, explicit errors, session/agent/origin/path checks with tests | Full constraint engine, comprehensive health/rebase diagnostics, untrusted archive/plugin boundaries, richer capability/privacy policies |
| 18: performance | Event/decode budgets and callback tests exist | Reproducible long-session benchmarks, incremental compile telemetry, device/latency/xrun matrices and published measured budgets |
| 19: testing | Rust, host/storage/HTTP, client logic suites and prior browser workflow | Full property/fuzz/interop/migration/recording/plugin matrices and all seven golden scenarios |
| 20: implementation plan | Stage sequence and epic identifiers retained in baseline; roadmap and this audit separate targets from evidence | Finish remaining stage-0 contracts before declaring stage 1 complete or prioritizing external plugins |
| 21–22: API examples/open decisions | Current wire is `docs/contract.md`; original examples/defaults preserved as design targets | Full version/capability negotiation and evidence-backed shell/backend/encoding qualification |
| 23–25: DoD/risks/checklists | Original gates restored to baseline; unresolved items enumerated below | First-usable-version DoD remains open; no implicit completion from passing current tests |
| Appendices A–E | Model/operation/diagnostic contracts, reference index, and ten design priorities included in baseline | Conceptual type skeletons still exceed current implementation; reference licenses require pin-specific verification |

Paths without directory prefixes in the table refer to `crates/core/src/`.

## Implemented in this audit: stable-ID local note edits

Original motivation: shared source edits must continue to propagate to untouched notes in locally varied repetitions (sections 2.2 A, 6.7, 7, 13.4, 20.1).

Previously `editNotes(scope="instance")` stored a full-pattern `ReplaceSlot("notes")`. Editing one note consequently hid all later source-note changes in that instance. Two failing regressions reproduced hidden source updates/additions and resurrection of a deleted locally edited target.

New edits store a typed `patchNotes` operation with only updated notes, added notes, and removed IDs. The complete submitted editable pattern is compared with the target's modifier/legacy-override baseline; previous note patches are normalized into at most one final patch. Cancelled edits release their IDs rather than leaving stale dependencies. Untouched source notes continue to change/add/disappear normally. Updated notes retain source IDs and modifier-origin metadata.

Additional boundaries implemented and tested:

- Missing updated/removed IDs, ambiguous effective IDs, or local-addition/source-ID collisions produce `RebaseConflict` during snapshot/render, rather than silent retargeting or duplication.
- A healthy instance remains explainable/editable when another instance has an unresolved patch.
- Commands accept only notes inside the visible first pattern, bounded by the lesser of definition and instance length. Imported patch records must fit the definition.
- `patchNotes` is internal to instance overrides, not a reusable modifier or proposal operation. Both canonical import validation and client contextual schemas enforce the distinction.
- Existing explicit/legacy `ReplaceSlot` data is preserved. No historical edit intent is guessed during load.

**Limits:** updates own an entire note record, not individual fields. The same edited note's future source properties stay overridden; per-field merge is not implemented. Existing whole-pattern overrides still hide their source range. A source change may save an unresolved canonical state; dry-run now reports affected note-target failures but is not a full rebase/compile gate, and render/export rejects unresolved output. General selector rebase UI remains open. Corrective local edits can replace the patch, or undo can restore the source. The feature does not complete all of section 7 or stage 0.

## Historical local-note implementation verification

| Gate | Result |
|---|---|
| Rust workspace tests, default features | **66 passed**, including 12 new note-edit/boundary regressions |
| Rust Clippy, workspace/all targets/default features, warnings denied | Passed |
| Rust formatting | Passed |
| Rebuilt core and audio executables | Passed |
| Host/storage/HTTP suite | **28 passed**, including new edit/source/export/import/conflict scenario |
| Client logic tests | **20 passed**, including patch schema and context restriction |
| Strict TypeScript + Vite production build | Passed; existing upstream Zod annotation warnings remain non-blocking |
| Independent manual HTTP driver | Local pitches `[49,77,68,66]`, peer `[61,77,68,66]`; non-silent WAV **13,165,760 bytes**; missing-target render HTTP **422/RebaseConflict**; healthy-peer query; undo followed by database close/reopen preserved result |

The manual driver used a disposable database and ephemeral loopback port, the actual HTTP handler/service/Rust workers, and an independent sequence outside the test runner. It did not alter user projects. Temporary QA artifacts were removed.

Rust/TypeScript language servers are unavailable; compiler/Clippy/strict TypeScript are the static checks, not a claimed clean LSP result. Native-feature, browser, physical audio-device, Windows, audible-quality, security-audit and long-session qualifications were **not rerun in this audit**. Earlier evidence remains historical and is labeled separately in [verification.md](verification.md).

## Follow-up implemented: pre-render note-target dry-run

`change.dryRun` now evaluates affected instances before/after the proposed command through the existing note resolver. It reports missing, ambiguous, and colliding note IDs without parsing error strings, definition revisions, first-loop note counts, advisory resolution actions, dependent definition/track IDs, and before/after instance preview ranges. A shared-source edit checks all dependent instances. A healthy local edit is not blocked by an unrelated unresolved neighbor. Corrective edits and deletions can report conflict → clean/absent.

The original `executable` remains structural save validity. The additional `noteResolution.ready` covers only affected first-loop note patterns, not the complete audio renderer. Stale/structurally invalid commands omit speculative reports. No source mutation, auto-retargeting, permission expansion, or implicit approval occurs. Client schemas preserve the optional reports while accepting older responses. Existing diagnostics still expose candidate failures without requiring a UI redesign.

Impact is instance-derived: it is not a complete changed-entity list and excludes audio-only regions, uninstantiated entities, pre-roll/tails, graph paths, and cache invalidation. Creation-preview IDs are provisional. The detailed contract is in [contract.md](contract.md#scoped-dry-run-impact-and-note-resolution-reports).

Verification for that follow-up: **77 Rust tests (11 new), 29 host/storage/HTTP tests (1 new), 23 client tests (3 new)**, Rust fmt/Clippy (workspace/all-targets/default features), rebuilt workers and strict TypeScript/Vite build passed. A focused independent Oracle review found no blockers within the documented scope.

An independent disposable HTTP driver observed source deletion report `clean → conflict` for `note-chorus-root`, impact on chorus instances A/B/C with beat ranges 24–32, 56–64 and 120–128, a corrective edit reporting ready, a healthy neighbor remaining ready, and stale-base rejection without a speculative report. Exported canonical state stayed structurally identical before/after dry-runs; the still-unmodified project snapshot remained usable. Temporary QA data was removed. No physical-device, native-feature, browser/visual or musical-quality qualification is claimed; LSPs are unavailable and compiler/Clippy/TypeScript supplied static checks.

## Follow-up implemented: persisted slot bindings and explicit rebase

New reusable `replaceSlot` attachments store `Instance.modifierBindings` keyed by modifier ID. Each binding records source definition ID/revision and each selector's range, ordered canonical note IDs, and cardinality. The current selector subset is whole-source `"notes"` or named canonical slots. Both snapshot and editable-note paths enforce the same guard; source revision, target set/count, selector inventory, or range drift requires explicit rebase rather than automatic retargeting.

`rebaseModifier {instanceId,modifierId}` is a human-only, expected-project-revision-bound typed command. It is dry-runnable and updates binding metadata only. Repeating apply does not rebind; remove clears the attachment metadata and duplicate preserves it. Structural import validation permits historical stale anchors but rejects malformed records. Legacy absent bindings preserve old playback and are exposed as `unboundModifierIds`, not silently classified as verified. Explicit rebase upgrades a legacy slot attachment.

This is deliberately Strong-only and conservative: even a source edit outside the replaced slot advances the source revision and requires rebase. Non-slot modifiers are unchanged. The binding refers to canonical source notes, not preceding-modifier-generated effective notes. Arbitrary selector ASTs, Adaptive/Snapshot modes, general application entities, and a dedicated UI remain unfinished; the subsequent opt-in ordering DAG is described below. **Old cores may drop the new metadata; downgrading bound projects is unsupported.** Full required-feature negotiation remains a separate storage gate.

Validation for that follow-up: **89 Rust tests (12 new), 30 host/storage/HTTP tests (1 new), 27 client tests (4 new)** passed, along with Rust formatting/Clippy (workspace/all targets/default features), worker rebuild and strict TypeScript/Vite build. Existing upstream Zod annotation warnings remain non-blocking. An Oracle design consultation and focused implementation check found no blocking deviation within this scope.

Independent actual HTTP QA created/applied a slot modifier, changed its source, observed a blocked render (422 `RebaseConflict`), closed/reopened the SQLite-backed service with the stale binding intact, dry-ran a non-mutating rebase, rejected an old project revision (409), explicitly rebased source revision 0 → 1, rendered a non-silent **13,165,760-byte WAV**, restarted the service again, and observed the persisted binding plus replacement pitch **80**. The driver used an isolated temporary store, removed afterward. No browser/visual, physical audio-device, Windows, or native-feature qualification is claimed; LSPs are unavailable and compiler/Clippy/TypeScript are the static gates.

## Follow-up implemented: modifier access algebra and ordering DAG

An optional `Instance.modifierOrdering` now enables a bounded explicit DAG. Absent metadata preserves legacy order; explicit empty means independent operations only. The shared planner validates and resolves both canonical and runtime paths, rejects malformed edges/cycles/undeclared hazards, and uses stable-ID topological tie-breaking. Directed paths satisfy dependencies; coincidental topological positions do not. Conservative read/write resources come from actual operation semantics and are exposed by `project.explain.modifierPlan`.

`setModifierOrdering` replaces the graph under existing human/revision rules. Optional ordering on a new `applyModifier` installs attachment and dependencies atomically. Remove prunes incident edges and rejects loss of required surviving order; duplicate preserves metadata; rebase does not change order. Local overrides remain fixed-last. Same-slot/parameter writes remain hard conflicts. Explicit mode also prechecks repeated IDs across every `AddNotes` operation, including within a single modifier and local overrides; this closes the coverage gap found during Oracle review.

Verification for that follow-up: **108 Rust tests (19 new), 31 host/storage/HTTP tests (1 new), 31 client tests (4 new)** passed. Rust fmt, Clippy workspace/all targets/default features, worker rebuild, strict TypeScript and Vite build passed. Existing upstream Zod warnings are non-blocking. Regression coverage includes transitive paths versus shared successors, clamp-sensitive velocity ordering, mixed-modifier hazards, additive gains, duplicate additions, explicit-empty versus legacy, graph bounds, attachment/removal, and import round trips. Existing fixture value types were extracted into an internal helper module to keep the fixture file within the repository's size policy without changing fixture content.

An independent HTTP driver observed an added note at pitch **60** in legacy transpose-then-add order and **72** after declaring add-before-transpose. An unordered dry-run returned `ModifierOrderRequired`; a cyclic apply returned HTTP **422** and preserved HEAD. Both renders were non-silent **13,165,760-byte WAVs** with different content. After service/SQLite restart, the explicit graph, plan and exact post-change WAV were restored. Temporary QA data was removed. No physical-device, native-feature, browser/visual or musical-quality certification is claimed; LSPs are unavailable and compiler/Clippy/TypeScript are the static gates.

The new graph is opt-in and capped at 256 modifiers/4096 edges. It covers current reusable operations and conservative instance resources, not arbitrary selectors or a signal graph. Gain compatibility is domain-level rather than bitwise floating-point equivalence; provenance follows execution order. Effective-note validity requires the separate final-output layer below. Old-core downgrade remains unsafe because it may discard the graph.

## Follow-up implemented: final effective-note identity and scalar validation

The core now validates actual emitted notes after modifier/override execution, repetition and cropping, and validates IDs again across the aggregated snapshot. Duplicate IDs, non-finite pitch, invalid normalized velocity and invalid final timing fail with `InvalidProject` before response serialization. This closes source-versus-addition and unaffected-note-versus-replacement collisions in both ordering modes, as well as finite-operand transpose overflow. The standalone renderer also rejects duplicate effective IDs independently of the core.

The check is deliberately on final emitted events: transient values replaced before output and wholly cropped-out notes do not fail this new layer. Existing earlier source/precondition/ordering/binding checks are unchanged. Fractional pitch and clamped large finite velocity factors remain supported. Scoped dry-run and explanation use per-instance validation; full snapshot additionally catches cross-instance ID collisions, including collisions possible with the existing delimiter-based encoding. IDs are rejected, not automatically remapped; changing that encoding is future work.

Verification for that follow-up: **122 Rust tests (14 new)** and **33 host/storage/HTTP tests (2 new)** passed, along with Rust formatting, Clippy workspace/all targets/default features and rebuilt core/render executables. New cases cover duplicate source/slot/global IDs, NaN/Infinity/velocity/timing, transient repair, crop/repeat, invalid proposal preview without changing main, ordinary fractional-pitch rendering, and the standalone renderer boundary. Client/UI files were unchanged; their prior tests/build were not rerun or re-certified in this backend-only follow-up.

An independent real HTTP driver exercised both a source-ID collision and overflow from two finite maximum transposes. In each case, dry-run reported note resolution not ready without changing state; snapshot/render returned **422 `InvalidProject`**; exported unresolved state survived a service/SQLite restart unchanged; undo restored a non-silent **13,165,760-byte WAV**. Temporary project data was removed. Rust LSP is unavailable, so compiler/Clippy supplied static checks. Physical-device, native-feature, browser/visual and musical-quality qualification were not performed.

Valid finite notes alone do not guarantee representable oscillator frequency, phase, mixed samples or encoded audio. The subsequent renderer boundary below handles that distinction without imposing a new MIDI-only clamp.

## Follow-up implemented: renderer numeric safety

The offline renderer now checks frame conversions, tonal frequency/phase, derived gains, post-filter tracks and final master samples; WAV encoding independently rejects NaN/Infinity before quantization. Finite PCM clipping and gain underflow-to-zero remain supported. Scratch memory is reused without carrying samples across tracks; source resampling uses checked integer indexing and preserves source-end silence. The callback path is unchanged.

Output allocation is capped at **8,640,000 stereo frames** and uses fallible reservations. This matches 180 seconds at the HTTP profile's 48 kHz, while higher-rate library callers receive a shorter maximum duration. It bounds requested render buffers, not total process memory/assets or CPU work. Derived checks skip unexercised mute/solo/empty paths and pitch-independent voices, while global input validation remains intact.

Latest verification: **139 Rust tests (17 new)** and **36 host/storage/HTTP tests (3 new)** passed, with Rust fmt, Clippy workspace/all targets/default features and rebuilt workers. Regressions cover frequency overflow/underflow, phase overflow from finite frequency, gain/PCM/mix overflow, direct WAV non-finite rejection, finite clipping, buffer reuse, half-frame rounding, frame cap and fallible allocation. HTTP tests also import extreme finite float PCM and verify that failed rendering creates neither WAV cache files nor render rows, leaves HEAD unchanged and recovers after undo. Oracle design/final implementation checks found no blocking deviation in this scope.

Four WAV fixtures captured before the changes matched SHA-256 exactly after rebuilding on the current Linux target: fractional tone, cropped tone, pitch-independent kick and resampled stereo PCM. Independent HTTP QA observed finite snapshots followed by 422 `AudioError` for frequency overflow/underflow, unchanged cache/HEAD across failures and restart, and undo recovery to non-silent **13,165,760-byte WAVs**. An exact 180-second render returned **34,560,044 bytes**; moving its end past the host limit returned **413 `ResourceLimit`**. Temporary data was removed. Client/UI files were unchanged, and browser, physical-device, native-feature and Windows qualification were not rerun. Rust LSP remains unavailable; compiler/Clippy supplied static checks.

## Next executable priorities

1. **E-006 / G-001 compatibility:** add versioned native export/import and required-feature checks so unsupported readers cannot silently drop binding/ordering semantics. Preserve deliberate legacy reads and reject unknown required capabilities before project mutation.
2. **B-003/004/008, D-005:** extend the implemented canonical-slot subset to general selectors/application records, richer rebase policies and full impact reporting.
3. **A-004/007, D-001/002:** proper tempo/meter and nested FlowGroup/AbsoluteGroup composition with duration and part/parameter bindings, guarded by deterministic fixtures.
4. **D-006, F-002/007:** incremental resolve equivalence, actual compiled graph and bounded read-ahead streaming. Basic stage-1 streaming must not be deferred merely because advanced streaming also appears in stage 2.
5. **F-005/006, H-001:** native live editor-control integration, device selection and real Windows/Linux desktop/audio qualification.

Recording, plugins, collaboration and ARA remain later work. Do not start those to avoid finishing the structural/review/playback gates above.
