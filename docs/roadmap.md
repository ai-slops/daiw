# Implementation roadmap

The [original design baseline](design-baseline.md) is the target; [progress.md](progress.md) maps it to current code and outstanding gates. Stage 0 remains partial and stage 1 is not signed off.

Completed: identity-based instance note patches, scoped dry-run note-target/impact reports, Strong-only slot bindings with explicit rebase, current-operation read/write algebra with an opt-in ordering DAG, final emitted-note checks, renderer numeric guards with bounded output allocation, versioned native exchange, and bounded note selection. Legacy ordering is preserved; invalid numeric output is rejected rather than sanitized. Next: general selector/application policies and full impact, nested composition/time, then incremental compilation/streaming and native-device qualification. Older cores must not save bound or explicitly ordered projects by stripping compatibility metadata. Basic streaming and tempo/meter requirements remain stage-0/1 obligations even where their advanced extensions are grouped below under stage 2.

## Stage 0: Headless domain foundation

Native read-ahead follow-up completed: verified-WAV disk worker, bounded inline ring, generation-safe frame controls, allocation-free callback and optional CPAL/CLI adapter. Remaining runtime work includes desktop native-control integration, signal graph/live DSP compilation, recording and actual device/background/long-session qualification. See [native-streaming.md](native-streaming.md).

Browser-streaming follow-up completed: bounded PCM ranges and scheduling, revision-bound metadata, A/B preparation/transitions, explicit underrun behavior and bounded host verification leases. Remaining runtime work includes native read-ahead/streaming, signal graph/DSP compilation, background/browser/device qualification and longer-session policy. See [browser-streaming.md](browser-streaming.md).

Output-streaming follow-up completed: bounded block DSP/WAV file generation and verified backpressured HTTP delivery. Remaining streaming work is browser/native playback and RT read-ahead, with long-session policy and physical-device qualification still required. Signal routing/DSP compilation remains a separate gap. See [output-streaming.md](output-streaming.md).

Offline source-streaming follow-up completed: direct binary PCM decoding, managed integrity-checked caches, bounded file reads and larger decoded sources. Remaining streaming work is output transport, browser/native long-session playback and real-time read-ahead; signal/DSP compilation and large-session qualification also remain open. See [offline-streaming.md](offline-streaming.md).

Instance-resolution follow-up completed: bounded post-modifier pattern reuse with live guards, exact work replay and separate human-only diagnostics. Next substantial runtime work is disk streaming and signal/DSP compilation; final-event reuse and measured large-session profiling are also unfinished. See [instance-resolution.md](instance-resolution.md).

Incremental-resolution follow-up completed: worker-local source-composition reuse, content-based dependency invalidation, deterministic bounded LRU and human-only cache diagnostics. Remaining incremental work includes operation/instance-level reuse, signal compilation, streaming and measured large-session budgets. See [incremental-resolution.md](incremental-resolution.md).

Nested-note follow-up completed: child placements, bounded DAG resolution, stable nested IDs/provenance, source/local editing, descendant binding guards and composition UI. Next work remains broader selector/application policies and incremental compilation/streaming; part/lane bindings, nested routing and flow graphs are not implied by this subset. See [composition.md](composition.md).

Timing follow-up completed: validated step/linear tempo maps, independent meter maps, forward/inverse conversion, mapped render/seek/loop and a usable point editor. Next structural work is nested composition and broader selector/application policies; incremental compilation and streaming follow. See [timing.md](timing.md) for implemented scope and exclusions.

Follow-up completed: versioned native exchange/required-feature rejection, UI file import, and a bounded nonrecursive SelectionHandle subset with source/local transpose and velocity transforms. Next work is broader selector/application policies, tempo/meter maps and nested composition, followed by incremental compilation/streaming. Storage hardening still includes decoded-cache authentication, portable media archives and explicit migrations. See [the current exchange contract](native-exchange.md); do not downgrade by stripping the envelope or feature metadata.

The baseline is a deterministic shared-chorus fixture, typed commands, source versus local edits, explicit modifier conflict handling, provenance, dry-run, proposals, revision checks, and a headless CLI. Regression scenarios must cover propagation, conflicting slot replacement, target drift, stable save/load results, and partial approval dependencies.

## Stage 1: First desktop-oriented vertical slice

The current implementation work targets:

- Rust domain authority behind a local service with a versioned JSON interface.
- SQLite snapshot transactions, command journal, autosave, recovery, and undo/redo.
- Arrangement, piano roll, composition inspector, source/instance scope, browser, mixer, and health views.
- Synthesized instruments, sample regions, WAV/AIFF/FLAC import, offline preview/master export, transport, and A/B audition.
- Local natural-language proposal planning plus a provider-neutral typed-operation boundary.
- Explicit dry-run, separate proposals, semantic changes, and human partial approval.
- Sandboxed desktop shell and platform audio adapters.

Do not equate a implemented adapter with a certified platform. Release acceptance still requires actual Windows/Linux device tests, restart/crash tests, cache rebuild, and desktop input/accessibility validation. The verification ledger is authoritative about which gates were run.

## Stage 2: General DAW core

### Recording and recovery

1. Add `RecordTarget`, `RecordingSession`, `Take`, and `TakeGroup` with latency metadata.
2. Capture audio/MIDI into append-safe chunks. Persist checksums and completed-frame counts outside the real-time callback.
3. Implement monitoring, punch, count-in, and loop recording.
4. Introduce non-destructive `CompSegment` selection and crossfades.
5. Kill the recording process at randomized block boundaries and recover only verified frames.

**Exit gate:** real-device recording and comping on both platforms, calibrated latency, no corrupted-tail playback after forced termination.

### Editing, time, and signal graph

1. Generalize tempo maps to step and linear-ramp segments with exact beat domains and inverse conversion tests.
2. Add warp maps, stretch algorithms, trim/slip/split, crossfades, and derived-asset recipes.
3. Generalize track-strip routing into typed ports, bus/aux/send/sidechain nodes, and explicit delayed feedback.
4. Add automation, modulation, and per-note expression as separate contributions.
5. Compile sample-accurate schedules and latency compensation at merge points.
6. Introduce bounded disk read-ahead and telemetry, rather than loading all source PCM.

**Exit gate:** PDC impulse fixtures, full/incremental resolve equivalence, variable-buffer callback tests, and cache-independent bounce provenance.

### Project management

Add backup-first schema migrations, portable archives, garbage collection using history reachability, project templates, and richer constraint/health tooling. Never ship a migration that only works by replaying old commands.

## Stage 3: External plugin ecosystem

Dependencies: device parameter schemas and compiled signal contracts must be stable first.

1. Build an isolated scanner with timeout/crash reporting and allow/block policy.
2. Integrate CLAP through a small audited adapter.
3. Add VST3 and, optionally on Linux, LV2.
4. Persist descriptor fingerprints, schema snapshots, opaque state, latency, and tail metadata.
5. Open native plugin GUIs in independent windows; verify DPI, focus, keyboard, and IME.
6. Preserve missing plugins as explicit placeholders with frozen fallback when available.
7. Add optional plugin bridges only after measuring IPC/latency tradeoffs.

**Exit gate:** instrument/effect compatibility matrix, sidechain, dynamic latency, state restoration, sample-accurate events, scanner crash isolation, and both-platform native-window QA. No proprietary plugin binaries in source control.

## Stage 4: Interchange, collaboration, and advanced workflows

1. DAWproject and Standard MIDI File import/export with explicit loss reports.
2. Semantic branch merge, dependency-aware review, object locks, and conflict resolution.
3. Opt-in authenticated TLS remote render with project-specific capabilities.
4. Launcher scenes sharing the same component definitions, plus arrangement capture.
5. MIDI 2.0/UMP, control surfaces, timecode/video, and external synchronization.
6. ARA-style document-level integration without exposing canonical mutation to plugins.

**Exit gate:** round-trip fixtures report losses, concurrent approval never bypasses stale-base validation, and all remote operations are opt-in and audited.

## Cross-stage rules

- Keep original assets immutable and project meaning independent of GUI state.
- Keep canonical, effective, and real-time representations separate.
- Never silently choose ambiguous targets or resolve conflicting writes by array order.
- Do not add a generic scripting engine or network access to the audio thread.
- Native plugin work must not displace tests for structural reuse and human approval.
- Every new schema feature needs an example, error case, migration story, and test fixture license.
