# AI-friendly structured DAW: implementation design baseline

Source: the user-supplied **AI 친화적 구조화 DAW 구현 설계서**, dated **2026-09-01**, draft v0.1, subtitled “Definition-Instance-Modifier 기반 프로젝트 모델과 인간 검토형 AI 편집 시스템”.

This is an English requirements digest and traceability index of that design, **not a verbatim translation and not a claim of implementation**. It preserves the original section numbering, architectural decisions, staged acceptance gates, and deferred scope. Detailed Rust-shaped examples in the original are conceptual contracts, not the current wire schema. [contract.md](contract.md) documents the actual supported interface; [verification.md](verification.md) records evidence. [../DESIGN.md](../DESIGN.md) remains the UI visual system.

## 1. Purpose, scope, and assumptions

Build a project-centered DAW in which humans make musical judgments and AI performs safe, precise structural edits. Initial platforms: Windows and Linux. Keep macOS possible without making it an initial release gate. The desktop web client must not own canonical music state.

The initial scope includes MIDI/audio playback, shared structural repetition and variation, built-in devices, project persistence, typed queries/commands, and proposals with diff, preview, and partial approval. Recording/comping, advanced audio editing, plugins, collaboration, DAWproject, and ARA are later stages.

Mandatory means required for architectural/product integrity; prohibited means not an implementation shortcut; recommended defaults may change with evidence. Non-goals: automatic complete-song generation, immediate commercial-DAW parity, opaque model representations as project truth, unrestricted runtime scripts, AAX/AU/VST2/32-bit bridges, initial video/notation/DDP/spatial mastering, and promises of lossless foreign-project round trips.

## 2. Product and user journeys

- **Shared chorus:** extract drums/bass/harmony/vocals into one eight-bar definition; place three instances; compose open-hi-hat, harmony, final-fill, and bass-octave modifiers. Source edits propagate; missing modifier targets require rebase review.
- **Scoped AI edit:** select chorus two, inspect bass/kick roles and signal paths, dry-run impact, edit a proposal variant, render pre-roll/tail, compare loudness-matched A/B, accept EQ but reject automation.
- **Project organization:** diagnose duplicate sources, orphan definitions, excessive overrides, missing dependencies, and irreproducible renders; propose structure-only refactoring and verify render equivalence before adoption.
- **Later recording/comping:** preserve immutable takes; represent comp as source-range selections; review alternative segments without destroying recordings.

Success measures sharing, predictable edit scope, enforced AI boundaries, understandable semantic changes, partial acceptance, stable save/reopen results, RT isolation, and explicit dependency failures, not subjective generated-music scores.

## 3. Requirements and priority

| Priority | Required scope |
|---|---|
| P0 | Create/open/save/autosave/recovery/archive/recent projects; exact beat time, tempo/meter, loops/markers/sections/snap; play/pause/stop/seek/count-in; MIDI notes/velocity/duration/channel/group/bend/CC/basic expression; selection/move/copy/split/trim/basic stretch/quantize/transpose/undo; WAV/AIFF/FLAC import, regions/gain/fades/resampling/basic streaming; built-in instrument, gain/pan/filter or EQ/meter; routing/master/mute/solo; preview/master render |
| P1 | Audio/MIDI recording, punch/loop/monitoring/latency; takes/comp; crossfades/slip/warp/stretch/consolidate; automation modes and modulation; bus/aux/send/sidechain/parallel processing/PDC; stem render |
| P2 | Plugin scanning/state/parameters/presets/GUI/missing handling; MIDI/stem/DAWproject interchange and loss reports; hardware I/O and synchronization |
| P3 | Launcher/performance capture, collaboration, advanced sync, video/timecode, spatial layouts/rendering |

Some cross-cutting P0 acceptance requirements span multiple epics. A later roadmap grouping must not silently waive basic stage-1 streaming, tempo/meter, or persistence obligations.

All logical objects need stable IDs, separate immutable revisions, provenance, reference integrity, explicit conflicts/rebase, review state, constraints, and dependency diagnostics. AI receives progressive purpose-built queries rather than full project dumps; every mutation is typed, revision-bound, dry-runnable, scoped, and reviewable.

Saving is transactional; migration requires a backup/checkpoint; missing sources/plugins/selectors cannot silently become defaults. Deleting all caches must leave canonical reconstruction possible. RT callbacks prohibit allocation/deallocation, blocking locks, I/O, logging, database/network/model calls, or high-level resolution. Preserve intra-block sample offsets. Compile off-thread and swap safely at block boundaries. Measure xrun, callback/compile/render time, cache hits, and underruns without exporting project content by default.

## 4. Architecture decisions D-001–D-014

| ID | Binding decision |
|---|---|
| D-001 | Canonical state is a semantic music model, not GUI state. |
| D-002 | Compose Definition–Instance–Modifier; do not create deep inheritance chains or implicit independent copies. |
| D-003 | Track is a facade over Part, Lane, RecordTarget, SignalNode, and VisualGroup. |
| D-004 | Original media and plugin-state blobs are immutable, content-addressed objects. |
| D-005 | Human and AI clients use the same typed command/validation boundary. |
| D-006 | AI cannot write raw project JSON, SQLite, blobs, or source files. |
| D-007 | Proposal, review, preview, partial approval, and reviewer identity are first-class history. |
| D-008 | Resolve high-level canonical state into effective state, then compile immutable PlaybackSnapshot. |
| D-009 | Native format preserves full semantics; DAWproject is interchange, not the native format. |
| D-010 | Built-in devices and an end-to-end vertical slice precede external plugin hosting. |
| D-011 | Project knowledge is not implicitly external training data. |
| D-012 | Core is a headless service/library; desktop, CLI, and AI are clients. |
| D-013 | Store current state and immutable revisions plus journals/checkpoints; command replay is not the sole recovery path. |
| D-014 | Ambiguous, Missing, Conflict, and Unsupported are explicit results, never hidden heuristics. |

Changes to these decisions require an ADR explaining consequences and migration cost. The initial ADR is currently embedded in [architecture.md](architecture.md); the original dedicated `docs/adr/0001-core-project-model.md` artifact remains to be established.

## 5. System model

Maintain connected asset, composition/time, signal, history, and project-knowledge graphs. Distinguish Canonical, Effective, and PlaybackSnapshot types. A common entity header identifies logical ID, revision, entity type, schema version, display name, roles/labels, creator/time, and provenance. UUIDv7 or equivalent stable 128-bit identity is recommended; content-derived revision, command, transaction, and commit IDs are distinct types. Paths, names, indexes, runtime pointers, and OS handles are not logical identity.

## 6. Canonical entities and invariants

| Entity family | Required information and meaning |
|---|---|
| Asset / DerivedAsset | Blob hash, media type/size, native audio rate/frame count/layout/encoding, origin/license/integrity; sources, processing recipe, renderer/version, reproducibility: Deterministic/Equivalent/Opaque/Unavailable |
| Time | Rational BeatTime with bounded denominator, LocalBeat/ProjectBeat, integer SampleTime and AssetFrame; explicit tempo/warp context; report import quantization error |
| Content | Note, Audio, Automation, Control, Composite, Generator; stable event IDs, onset/duration, pitch not restricted to integer MIDI, velocity/release/articulation/expression/role |
| AudioRegion | Immutable source, native frame range, local start, duration policy, warp, gain/fades/channel mapping |
| Part / Lane / Binding | Musical role, timeline/display lane, and instance-local routing are independent |
| ComponentDefinition | Duration, meter assumption, nested local placements, parts/lanes, semantic anchors, replaceable slots, exposed parameters, constraints |
| ComponentInstance | Revision-selected definition, placement, time/pitch transforms, parameter/part bindings, modifier applications, separate local overrides |
| Duration policy | FollowEffectiveContent, FixedCrop, LoopToFit, StretchToFit, ErrorOnChange |
| Arrangement / Section | Root component, tempo/meter, markers, role/occurrence/parent; explicit AbsoluteGroup overlays and FlowGroup sequential placements |
| ModifierDefinition / Application | Reusable parameterized operations/read-write declarations/dependencies; target instance, resolved selector bindings, order, enabled state, rebase status per application |
| SignalGraph | Typed nodes/ports/connections: audio, note event, parameter event, control, clock; channel compatibility, cardinality, legal delayed feedback, sidechains |
| DeviceInstance / Profile | Built-in/plugin/hardware implementation; port/parameter snapshots, opaque state, latency/tail/capabilities; semantic controls map to typed parameters without rewriting opaque state |
| Parameter contributions | Default → definition binding → local override → automation → modulation → per-note expression → live input; explicit Replace/AddNormalized/AddPhysicalUnit/Multiply/Min/Max/BipolarModulation |
| Recording / Take / Comp | Session input/clock/rate/block/latency metadata; immutable captured asset ranges; non-destructive segment selections and crossfade recipes |
| Knowledge | Intent, hard/preferred/temporary/informational constraints, scoped predicates/rationales, decisions/alternatives/reviews, principal/task/source/tool provenance |
| History | Command and expected base/resolved targets; ChangeSet/inverse/semantic diff; Commit parents/root state; named Variant and review purpose; IdentityMap for split/merge |
| Proposal / Review | Base, variant, operations, impact/previews; Draft → Planned → AppliedToVariant → Rendered → InReview → PartiallyAccepted/Accepted/Rejected/Superseded |
| RenderArtifact | Source commit, range/output/profile, dependency fingerprint, output asset, tail, loudness, reproducibility |

References carry ID plus expected revision/resolution policy. Never physically delete reachable history objects, overwrite originals, conflate reusable and local changes, silently order conflicting writes, ignore cardinality drift, or make caches necessary for opening a project.

## 7. Modifier composition

Prefer slots, then semantic anchors, stable IDs, semantic selectors, and only lastly raw positional selectors. Persist selector AST, base revision, resolved IDs, cardinality, binding mode (Strong/Adaptive/Snapshot), and resolution hash.

Operation domains: Structural (placements/layers/slots/duration), Musical (notes/phrases/transposition/voicing/articulation/fills), Performance (velocity/timing/groove/seeded humanize/length), Sound (parameters/presets/devices/modulation), Mix (gain/automation/send/pan/bypass), Routing (connections/part bindings/sidechains). Each declares reads, writes, preconditions, and output identity policy.

Precedence: definition defaults → reusable modifier → section modifier → local override → temporary audition. Within scope, use an explicit before/after dependency DAG and conflict algebra: disjoint additions commute; compatible additive deltas combine; transforms need meaningful order; two setters/slot replacements conflict; removal versus editing conflicts; routing validates port rules; multiple automation overlays need explicit combine mode/priority. Reject cycles.

Rebase states: Clean, TargetsChanged, Ambiguous, Missing, PreconditionFailed, Conflict, ManualResolution. Length changes obey duration and flow/absolute placement policies; explain effects on later sections, markers, automation, and absolute cues. Parameterized expressions are pure, typed, deterministic, bounded, without arbitrary scripts/network/files/time; humanization requires a seed.

Structural commands include create-from-selection, extract common definition/modifier, promote override, inline/detach/rebind/squash/rebase/consolidate. Declare whether sound is preserved and optionally compare offline renders.

## 8. Resolver and compiler

Pin commit/root/revisions; reject dependency cycles; expand nested contexts; bind parts/parameters; resolve selectors; order and validate operations; compose changes; map flow/absolute time; produce effective notes/regions/automation/routing with OriginPath; evaluate constraints; report impact; compile signal/time graphs.

OriginPath includes source revision, placement/instance/modifier chains, and operation ID. Effective IDs derive deterministically from source event, instance path, applied operation, and loop index. Impact includes directly changed objects, dependent instances/sections/signal paths, invalidated caches/previews, rebase/constraint results, and recommended preview ranges.

Cache keys include definition/modifier revisions, parameters, selector hashes, bindings/transforms, resolver version. Track inverse dependencies; incremental and full resolve must agree. Compiler converts beats to samples, validates ports/feedback, schedules nodes, plans buffer lifetimes/PDC/event indexes/stream read-ahead, and seals an immutable snapshot. On failure, keep the last valid playback snapshot, display its divergence from HEAD, and block export by default.

## 9. Time and transport

Separate LocalBeat, ProjectBeat, SampleTime, AssetFrame, WallClock, and PerformanceTime. Tempo supports positive, contiguous non-overlapping step/linear segments and deterministic forward/inverse conversion. Meter/bar numbering is independent. Warp points increase monotonically in both frame and beat with explicit algorithm/version/quality. Structural looping, content looping, transport looping, recording loops, and launcher repetition are different concepts.

Transport commands carry expected generation, action, optional target, and quantization; bounded queues preserve sample-accurate boundaries. Include play/stop/seek/loop/metronome/pre-roll/count-in; distinguish live and render transport. Later launcher activations reuse component definitions and can capture performance into arrangement under explicit lane ownership.

## 10. Audio engine

Keep UI, core/control/RT callback, disk/analysis/render workers, scanner, and optional plugin bridges separated by clear crash/API boundaries. Preallocate planar float buffers with variable rate/block size and general channel layouts; define denormal behavior. Start with single-RT-thread correctness. Calculate path latency and merge-point compensation; recompile on dynamic latency changes; include device tails.

Disk workers decode/resample into read-ahead rings, discard stale seek generations, and distinguish cache misses from underruns. Small one-shots may be resident; waveforms use multi-resolution caches. Own an AudioBackend abstraction: Windows WASAPI plus later ASIO; Linux PipeWire/JACK with ALSA fallback; CPAL is a prototype adapter, not the domain model.

Built-in target set: SamplePlayer (range/root pitch/ADSR/velocity/loop), BasicSynth (oscillators/ADSR/filter), GainPan, EQ, basic Compressor, Meter, Delay or Reverb. Provide full parameter schemas/profiles and deterministic state. The first slice may implement a subset but must be musically testable without external plugins.

Plugin order: built-ins → CLAP → VST3 → optional LV2 → ARA. Scan out-of-process with timeout/crash/blocklist/cache; preserve binary fingerprints, port/parameter snapshots, state, latency/tail, dialect, capability and version metadata. MissingDevicePlaceholder retains opaque state/automation/ports and optional frozen fallback. Start with separate native GUI windows; isolate live plugins selectively only after latency/IPC measurement. Offline rendering shares compiled graph semantics with live playback and records rate/depth/dither/layout/output/normalization/tail profile.

## 11. Command, query, and history API

Typed command envelopes carry separate command/idempotency IDs, expected commit, principal/task, scoped capability, and payload. Return revisions, IdentityMap, semantic diff, impact, and warnings. Interactive edits preview transiently then commit a semantic transaction. Query progressively with cursors; SelectionHandle pins expression, resolved revisions, scope, and expiry.

Workflow: BeginChangeSet → ResolveSelection/context → commands → DryRun → ApplyToVariant → Compile/RenderPreview → Review → Accept all/hunks → Commit.

Query families: project/arrangements/sections/role/entity/history/knowledge; range/effective/base-vs-effective/instance differences/event explanation; dependencies/usage/impact; signal contributors/path/device capabilities/parameters/latency/meters/analysis; constraints/health/unresolved/commit comparison/variants.

Command families: components/instances/sections/flow/transforms; modifiers/rebase/extract/squash; note/phrase/audio-region/fade/warp/comp; devices/state/parameters/connections/send/sidechain/automation/modulation; variants/change sets/previews/hunk approval/rejection/commit/merge/decisions.

Dry-run reports resolved commands, impact, semantic diff, hard failures/preferred warnings, conflicts/ambiguities, capabilities, compile scope, previews, and executable status. Semantic hunks cover notes, audio, automation, devices, routing, structure, knowledge with dependencies. Errors have machine code, explanation, related objects, expected/current state, resolution actions, and safe-continuation status; include stale/missing/ambiguous/cardinality/precondition/constraint/conflict/cycle/type/unsupported/permission/migration/corruption cases.

## 12. AI integration

AI plans and edits through the API but cannot judge subjective success, choose ambiguous targets silently, install plugins, delete originals, upload media, or commit protected HEAD by default. Persist task stages: Requested → Scoped → Planned → DryRunPassed → AppliedToProposalVariant → PreviewReady → InReview → Accepted/PartiallyAccepted/Rejected → Committed/Archived.

Retrieve selection, roles, source usage, effective differences, signal paths/capabilities, constraints/intents, unresolved issues, and allowed schemas incrementally. MCP may adapt the API, never define internal semantics. Natural language must lower to supported typed operations; unsupported semantic controls fail explicitly.

Deterministic analyzers include peak/RMS/LUFS/true peak, spectra/band energy, stereo correlation, onset density, alignment, silence/noise, and clipping diagnostics with algorithm/input provenance. They do not prove musical quality.

Review provides equal-range synchronized A/B/C, pre-roll/tail, loudness match with displayed gain, modifier audition, notes and ratings. Partial approval reconstructs dependency-closed hunks, rebases/revalidates changed bases, renders again when needed, and records reviewer evidence.

Default capabilities allow read/analyze/create-own-variant/mutate-own-variant/bounded-preview. HEAD commit, original deletion, plugin install, external network, and export destinations require separate authorization. Scope by project/branch/range/role/command. Project states draft/arrangement-locked/recording-locked/mix-review/approved/released have explicit policies.

Project text, names, presets, and tool output are untrusted data, not instructions. Keep credentials outside projects; disclose provider-bound data per task. Provider-neutral adapters own model invocation, validation, orchestration, context/cache management. Store Intent/Constraint/Decision/TaskSummary rather than requiring an entire chat transcript. Evaluate scope violations, invalid commands, ambiguity resolution, hunk acceptance, rebase preservation, health, repeatability, and retrieval cost.

## 13. UI and input

Retain familiar arrangement/piano-roll/mixer surfaces while making source/local/modifier/effective edit modes explicit. Double-click opens instance context; direct arrangement note edits default to local overrides. Linked instance duplication remains linked; independent copy is explicit. Show definition usage before source edits and badges for modifiers/local changes/conflicts/protection without relying on color alone.

Required views: arrangement/sections/markers; nested component/anchor/slot/parameter editor; composition inspector with bypass/solo/reorder/rebase; effective/base/change overlays; canvas/GPU piano roll with expression/exact time/ghost instances; waveforms/trim/slip/fade/warp/lineage/comp; mixer and routing graph with latency; layered automation; AI task/scope/plan/dry-run/diff/preview/approval; variants/dependencies; health and searchable asset/definition/modifier/device/preview browser.

Health explains blocking/needs-review/advisory issues, not an opaque score. Include missing/corrupt dependencies, selector conflicts, orphans/duplicates, nesting/override complexity, irreproducible renders, latency/clipping/streaming risks, unreviewed changes, migration/license/provenance gaps; offer explain/propose-fix/ignore-with-reason.

React/TypeScript is recommended for panels, with batched canvas/WebGPU/native rendering for dense editors and compact binary/shared-memory telemetry. Default shell direction is core-independent Chromium; compare Electron/Tauri/Qt in an explicit spike. Require keyboard actions/palette/configurable shortcuts, IME-safe composition, canvas accessibility tree, contrast/zoom/color-vision support, and visible undo/edit scope.

## 14. Persistence

Native working projects are directories, not live-edited ZIPs: manifest, SQLite WAL, content-addressed objects, logical asset names, plugin state/history/refs, rebuildable caches, transaction/recording temp, locks. Manifest declares format/version/project/app/state DB/HEAD/required features/hash/portability; database is authoritative for atomic HEAD.

Store typed canonical JSON initially with deterministic ordering and relational reference/search indexes; preserve current state, immutable entity revisions, commands/change sets/commits/parents/refs/proposals/reviews/blob metadata/cache indexes/migrations. Blob writes: temp → fsync/hash → atomic rename → transactional reference. Deduplicate; GC requires grace period and reachability from refs/proposals/review previews/checkpoints.

Finalize blobs before committing entity revisions/journal/HEAD atomically. Autosave checkpoints do not require user commit messages. Record append-safe chunks plus checksums/frame-count journals outside RT; recover only verified complete frames after crashes. Migration declares source/target/validation/tool version and makes a prior checkpoint; unknown required features fail or open read-only; preserve opaque plugin state.

Portable archives collect reachable sources, omit rebuildable caches/plugin binaries, optionally retain previews/frozen audio, and report missing external references/licenses. Extract to a working directory, validating traversal/size/depth/hashes. DAWproject/MIDI imports map supported content with explicit loss reports; exports flatten unsupported semantics with reports. Master/stem/part/port renders record source/profile. User window/panels/devices/shortcuts/credentials stay separate from musical project state; scrub private paths on export.

## 15. Technology and repository boundaries

Rust core and TypeScript/React UI are defaults; RT safety is not automatic in Rust. Isolate C/C++ SDK/unsafe FFI. Proposed domain crates: ids/model/time/commands/queries/resolver/modifier-engine/constraints/history/storage/asset-store/signal-graph/audio-engine/audio-io/dsp-builtins/plugin adapters/render/analysis/rpc/core-service/import/test-fixtures, with desktop/CLI/scanner/render clients. Do not create empty crates solely to match this list.

Model cannot depend on storage/UI/plugin SDK; audio engine cannot depend on DB/AI; plugin formats cannot leak into canonical types; clients use public RPC. Start with local versioned JSON request/response plus capability/schema discovery and binary telemetry; bind loopback by default. Support embedded, local daemon, headless render, and explicit authenticated remote modes.

Use typed IDs/time, deterministic collections/serialization, schema/examples/error cases per public command, migration fixtures, and documented unsafe boundaries. License choice, CLA/DCO, THIRD_PARTY/SBOM/license CI and fixture permissions are explicit work. Never copy incompatible GPL engine code into a permissive core. Check actual pinned licenses for DAWproject/CLAP/VST3/ARA/LV2/Clack/codecs/stretchers rather than treating design-era license observations as current audits.

## 16. Validation and health

Layers: schema → references → domain invariants → modifier resolution → constraints → graph compile → runtime capabilities → render diagnostics → hygiene. A saved incomplete edit may differ from playable state, but diagnostics and last-valid playback must make that explicit.

Validate duplicate/dangling IDs, cycles, cardinalities, duration/ranges/tempo/warp/parent relations, ports/layout/cardinality/feedback, missing devices/parameters/output reachability and PDC limits. Hard constraints block; preferred rules warn; overrides need explicit rationale. Lint duplicates, excessive overrides/chains, broad positional selectors, repeated intent, unused slots, and absolute-cue conflicts without automatic restructuring. Diagnose corrupt/missing assets, stale freezes, mismatched plugin binaries, restore failures, NaN/Inf/DC/clipping/tail cuts/underruns; numerical observations are not musical judgments.

## 17. Security and privacy

Treat imports, archive paths/metadata, MIDI text, plugins/profiles/presets, AI output, project prose, and remote clients as untrusted. Bound archive expansion/entity counts/blob sizes/schema depth/strings; do not auto-execute plugins or read arbitrary remote paths when opening projects. Scanner timeout/privilege policy, plugin fingerprints, credential isolation, typed scoped AI commands, approval for destructive/export/network actions, explicit remote bind/auth/TLS/ACL/audit, opt-in telemetry, and scrubbed crash reports are required boundaries.

## 18. Performance and observability

Qualify 44.1/48/96 kHz, multiple buffer sizes, seek/loop/swap during edits, many sources/devices/PDC paths; dense notes/lanes/waveforms and independent metering; large entity/instance/modifier/history sets and lazy large-asset indexing. Publish measured hardware/backend/configuration, not universal unmeasured thresholds.

Separate device-reported, measured round-trip, graph, and monitored-path latency. Measure full/incremental compile time/cache invalidation/swap delay and display saved-versus-playing generations. Structured non-RT spans cover storage/commands/resolution/compile/plugins/render/assets/AI; RT emits only bounded counters/events. Benchmark fixtures: dense-midi, nested-components, modifier-conflicts, long-audio-streaming, routing-pdc, rapid-edits, many-variants, corrupted-assets.

## 19. Testing and fixtures

Unit: time/conversion/warp/selector/operations/conflicts/graph/canonicalization. Property: inverse/save-load hashes, full-vs-incremental equivalence, commutative order, explicit noncommutative order, origin/reference integrity. Golden: debug export/diff/DSP/render/migrations. Fuzz: archives/importers/plugin boundaries/selectors/commands/recovery. Integration: GUI or agent → RPC → resolver → snapshot/render/review, restart/crash/missing dependencies. RT: allocation/deallocation/deadlines/block sizes/swap/sample offsets/seek-loop.

Seven required scenarios: shared chorus propagation; same-slot conflict; selector rebase after source split; note/device/automation partial approval dependencies; recording crash recovery; missing-plugin placeholder/freeze/restore; structure-only refactor render equivalence. Later-stage scenarios are not initial implemented features.

Use synthetic licensed fixtures first; then DAWproject official test-data and individually license-checked public sessions (for example Lorenzo's Music Ardour), LMMS examples, and stem datasets. Do not assume a repository license covers every song/sample or vendor proprietary binaries. Plugin matrices include instrument/effect, buses/sidechain, latency/state/presets/sample-accurate events/expression, GUI/DPI/crash/offline behavior.

## 20. Implementation stages and epics

| Stage | Scope and exit gate | Current assessment |
|---|---|---|
| 0 | Headless IDs/time/model/modifiers/anchors/slots/selectors/order/flow/resolver/origins/commands/diff/history/variants/debug CLI; shared chorus, propagation/conflict/rebase, deterministic save/load and origins | Partial: executable flat model and tests; full graph/selector/order/time model outstanding |
| 1 | SQLite/blobs/RPC/incremental resolve/snapshot/transport/platform backends/import/streaming/built-ins/render/recovery; core editors/review/AI tools | Partial vertical slice, not release sign-off; see ledger |
| 2 | Recording/monitoring/latency/punch/takes/comp/recovery; audio editing/warp/stretch/recipes; routing/automation/modulation/PDC/stems; migrations/archive/templates | Deferred |
| 3 | Device abstraction/scanner/CLAP/VST3/native GUI/state/missing placeholders/optional bridges/LV2 | Deferred |
| 4 | DAWproject/MIDI 2.0, semantic merge/review/locking, remote render, launcher capture, ARA, video/timecode/control surfaces | Deferred |

Stage 1 exits only when a user makes a short MIDI/sample song, shares repeated components with modifiers, distinguishes source/local edits, receives scope-safe AI proposals with impact, auditions A/B and partially accepts, reopens the same sounding result, rebuilds after cache deletion, and passes callback allocation checks. Actual Windows/Linux playback is required, not merely compilation.

Original issue inventory (retain identifiers for future tracking):

- **A Domain:** A-001 IDs; 002 headers/revisions; 003 canonical serialization; 004 beat/tempo/meter; 005 assets/content; 006 components/instances; 007 flow/absolute placement.
- **B Modifiers:** B-001 operation registry; 002 anchors/slots; 003 selector AST; 004 selector resolver; 005 read/write sets; 006 ordering DAG; 007 conflict algebra; 008 rebase; 009 common-definition extraction.
- **C Commands/history:** C-001 envelope; 002 transaction/change set; 003 diff; 004 inverse/undo; 005 commits/variants; 006 hunk dependencies.
- **D Resolver:** D-001 expansion; 002 parameter/part binding; 003 modifiers; 004 IDs/origins; 005 impact; 006 incremental cache.
- **E Storage:** E-001 directory/bootstrap; 002 SQLite; 003 blobs; 004 atomic transaction; 005 checkpoint/autosave; 006 migration; 007 debug export.
- **F Audio:** F-001 graph types; 002 compiler; 003 snapshot; 004 RT executor; 005 Windows backend; 006 Linux backend; 007 streaming; 008 sampler/synth; 009 gain/pan/filter/meter; 010 preview render.
- **G RPC:** G-001 negotiation; 002 queries; 003 command/dry-run; 004 change stream; 005 binary telemetry; 006 CLI.
- **H UI:** H-001 shell spike; 002 arrangement; 003 component editor; 004 piano roll; 005 browser/inspector; 006 mixer; 007 composition; 008 diff/review; 009 A/B; 010 health.
- **I AI:** I-001 scoped session; 002 retrieval planner; 003 plan schema; 004 tools; 005 proposal workflow; 006 partial approval; 007 prompt/data isolation.

Original dependency order: ADR → IDs/time/model → shared chorus → add-notes/transpose/replace-slot/set-parameter → resolver/export → conflict/rebase/property checks → commands/variants → SQLite → graph/synth/offline WAV → backend/UI → dry-run/diff/preview API → natural-language agent. Do not start from chat UI or skip domain obligations because a demo already renders.

## 21. API examples: specification versus current wire

Original conceptual methods include `project.getSummary`, `changeset.addCommand` with `CreateAndApplyModifier`, and `proposal.acceptHunks` bound to `expectedProposalHead`. Semantic examples transpose bass at `last-two-bars` only in chorus two, dry-run reports affected section/instances and preview tail, and approval selects EQ/width hunks while excluding automation. These names are **not current callable methods**; use [contract.md](contract.md) for `project.get`, `change.dryRun`, and `proposal.accept` until the richer API exists.

## 22. Open decisions and defaults

Shell: independent core + Chromium pending Windows/Linux IME/DPI/GPU/native-focus/import/accessibility/restart spike. Payload: canonical JSON plus relational indexes before measured CBOR optimization. Backend: own trait with CPAL/platform prototype. Plugins: CLAP then VST3 without format-shaped canonical data. Selectors: typed AST before string DSL. Collaboration: single-user branches/history before CRDT. Modifier expressions: small pure AST, not initial WASM/scripts. Event identity: stable note/automation IDs; dense control streams may use chunks. DSP: useful reproducible minimum. Native format: public schema/migrations/debug export; raw DB edits are not supported API.

## 23. First usable version: definition of done

- Model: persisted/resolved definitions/instances/modifiers; explicit source/local differences; selectors/resolved targets/base revisions; visible conflict/ambiguity; semantic diff/origins.
- Music: MIDI editing and sample regions; tempo/meter/arrangement; audible built-in instrument/effect; master and bounded preview renders.
- AI: progressive queries, typed changes, dry-run/proposal/preview/diff, partial acceptance; unapproved changes never enter main.
- UI: arrangement/component/piano-roll/inspector/basic mixer; shared/local/modifier/conflict distinctions; source impact; synchronized A/B.
- Engine/storage: RT tests, atomic snapshot handoff, transaction/autosave crash tests, cache-independent rebuild, physical Windows/Linux backend qualification.
- Delivery: shared-chorus/conflict/rebase/partial-approval CI, documented schemas/API, ADR/license inventory, licensed fixture provenance.

## 24. Risks and mitigations

Complex model → TrackFacade and simple local-edit default. Modifier complexity → explain/overlay/usage/lint/extract/squash. Selector drift → stable anchors/IDs plus cardinality/preconditions/review. Weak plugin semantics → profiles/capability checks/conservative previews. RT coupling → compiler boundary and instrumentation. Scope explosion → finish stages 0/1 before plugins/recording/collaboration. Refactor changes sound → deterministic ordering, identity maps, render equivalence. Native/web focus/IME/DPI → separate plugin windows and real-platform qualification.

## 25. Implementation checklist and next-work policy

- Repository: license and contribution policy, pinned toolchains/package manager, format/lint/tests, SBOM/license CI, dedicated core ADR, fixture license manifest.
- Model: ID/revision/hash/time/serialization/order/reference/deletion/schema policies.
- Modifier: operation registry, slot/anchor semantics, selector binding, conflict matrix, duration policy, rebase UX.
- Engine: RT coding guide/detector, snapshot ownership/swap, typed ports/layouts, backend abstraction, shared offline/live semantics.
- AI: stable query/command schemas, scoped capability, proposal isolation, dry-run/impact, hunk dependencies, preview/A-B, injection tests.

Evidence-backed next order: local note identity propagation → stable selector/application/rebase contracts → dependency ordering/impact → nested flow/absolute composition and temporal model → incremental resolver/streaming → native editor output and real-device qualification. Reassess at each completed slice; this is not authorization to mark entire epics complete.

## Appendices A–E: retained contracts and references

**A. Model skeleton:** ProjectState owns Project, entity store, HEAD; Project references arrangements, signal graph, knowledge, policy. EntityPayload discriminates assets/content/components/instances/modifiers/applications/arrangement/graph/devices/constraints/intent/decisions/proposals/renders. CommandHandler separates dry-run and transaction apply; Resolver returns EffectiveComponent for a fixed project/context; SnapshotCompiler produces PlaybackSnapshot from effective project/config.

**B. Operation skeleton:** semantic operations resolve targets, declare read/write sets and preconditions, compare conflicts, then apply typed effects. Initial enum families include add/suppress/replace/transform events, exposed parameters/automation, devices/parameters/connections, duration changes. No generic JSON Patch.

**C. Diagnostic contract:** stable cause-derived diagnostic ID, machine code, severity/category/message, related entity/time ranges, explanation, suggested actions, suppressibility. Ignoring with a reason persists for the same cause; a changed cause gets a new identity.

**D. Reference index (design references, not vendored code or verified current license claims):** [DAWproject](https://github.com/bitwig/dawproject), [CLAP](https://github.com/free-audio/clap), [VST3](https://steinbergmedia.github.io/vst3_dev_portal/), [Ardour](https://manual.ardour.org/), [Bitwig](https://www.bitwig.com/userguide/latest/), [Ableton](https://www.ableton.com/en/live-manual/), [REAPER API](https://www.reaper.fm/sdk/reascript/reascripthelp.html), [MIDI 2.0](https://midi.org/specifications/midi-2-0-specifications), [ARA](https://github.com/Celemony/ARA_SDK), [LV2](https://lv2plug.in/), [PipeWire](https://docs.pipewire.org/), [CPAL](https://github.com/RustAudio/cpal), [Clack](https://github.com/prokopyl/clack), [Lorenzo's Music](https://github.com/lorenzosmusic/with-you), [LMMS](https://github.com/LMMS/lmms).

**E. Decision priorities:** do not duplicate shared music; make differences small reusable modifiers; expose source/local scope; keep familiar tracks without making them the entire model; share typed validation across humans/AI; require proposal/dry-run/diff/preview/approval; separate canonical and RT state; preserve immutable sources and recipes; expose ambiguity/conflict; finish the structural vertical slice before external plugins.
