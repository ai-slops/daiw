# Verification ledger

This file records evidence, not aspirations. The supplied design remains the product target; the current repository must not be described as having passed every stage-1 acceptance condition.

## Latest follow-up: bounded DSP publication and retirement (2026-10-11)

- **363 all-feature Rust**, **122 host/API** and **106 client tests** passed. Full workspace all-feature build/Clippy, Rust formatting and TypeScript/Vite build passed; all **five** changed Rust files passed no-excuse checks and relevant LSP checks had no errors. Native-disabled hints and upstream Zod annotation warnings remain non-blocking.
- Red-first publication tests preceded implementation. Public regressions cover fresh initial/candidate reset with independent offline PCM equality, whole 5,000-frame invocation, build/rate/full failure preservation, queue activation boundaries, retirement saturation/deferred retry, endpoint ordering, explicit shutdown, borrowed scoped threads and sticky-fault recovery by replacement.
- `stats_alloc` measures zero allocations/deallocations around callback success, empty/EOF, ordinary and deferred swaps, saturation stalls, fresh/sticky fault and reset replay. Channel ownership remains bounded to active/pending/retired/deferred executors; caller/build ownership is additional. No unsafe code, dependency changes or native/UI adapter changes were introduced.
- The lead ran a separate public-API crate that checked old PCM/cursor after failed build, observed queued-not-active state, full-queue rejection, exact replacement PCM over **4,097 frames**, delayed adoption until control reclamation, zero callback allocation/free, positive control-side reclaim/shutdown deallocation and scoped callback ownership returned to control before teardown. Independent final review approved all ownership and failure branches.
- Ordinary endpoint destruction is not callback-safe. Enqueue success does not guarantee activation or concurrent endpoint liveness; disconnect detection is best effort. Replacements start at frame zero, with no crossfade, transport continuity, runtime rollback, source prefetch or device integration. Actual native/Electron qualification remains deferred by user direction.

## Prepared DSP execution boundary (2026-10-11)

- **356 all-feature Rust**, **122 host/API** and **106 client tests** passed. Full workspace all-feature build/Clippy, formatting and TypeScript/Vite build passed. Relevant DSP source/test LSP diagnostics had no errors; expected native-feature hints and upstream Zod annotation warnings remain. Rust no-excuse checks passed for all **23** changed files.
- Public API regressions prove owned compilation independent of snapshot lifetime, variable partitions including lengths above 4,096, resampling/filter parity, empty/EOF/reset behavior, source metadata/layout/channel/finite validation, event cap after expansion and zero-frame exceptions. Existing source-read sequence, duplicate tracks, file-backed bytes and WAV golden remain unchanged.
- Independent review found metadata could change between counting and construction and empty executors initially reported Running. Public compile now uses one successful lookup per asset across both passes and checks the constructed count; empty preparation/reset report Ended immediately. Stateful-provider and EOF regressions passed, and bounded re-review approved the fixes.
- Allocation instrumentation observes zero allocations and deallocations during success, EOF, a second-chunk fault, repeated fault and reset replay. Runtime errors are numeric Copy values; file I/O and string error conversion remain confined to the offline adapter. No hardware deadline or full process-memory guarantee is inferred from this test.
- The lead's external Rust crate compiled from a temporary snapshot, processed **7,001 frames** with caller partitions **1/3/4,097/5,000**, and matched offline PCM bit-for-bit for synthesis plus a 32-kHz stereo source. EOF tail and reset replay were exact. A late fault in a **4,097-frame** call zeroed the entire call, retained cursor **4,096** and remained sticky on repeat/reset replay.
- UI and device adapters were unchanged, so no browser visual or physical-device gate was rerun. Per user direction actual native/Electron qualification is deferred until suitable hardware/display access. Program hot swap/retirement, asynchronous source supply, incremental compilation, seek/loop state restoration and device callback hookup are still outstanding.

## Desktop native playback controls (2026-10-11)

- **344 all-feature Rust**, **122 host/API** and **106 client tests** passed. Full all-feature workspace build/Clippy, Rust formatting and strict TypeScript/Vite build passed. Relevant LSP checks had no errors; pre-existing agent-client Zod deprecation hints and upstream Zod build annotations remain non-blocking.
- Rust tests exercise bounded JSONL commands, actual file-worker/callback advance, EOF state, selected-device digest/duplicate-name policy, safe startup errors, and shutdown/fatal exit while stdin stays open. Node/Electron tests cover opaque tokens, trusted main-frame IPC, missing binary, revision drift, startup cancellation, slow-close preparation races, worker death/retry, token-preserving session release, IPC disconnects and pipe/protocol limits.
- Independent lifecycle reviews found and corrected orphaned startup workers, reload/backend overlap, broken-pipe failure paths, expiring/ambiguous device selectors, misleading EOF/startup states, latest-owner races, missing terminal notifications, and stale asynchronous UI errors. Final bounded re-review approved the contract.
- The lead's disposable integration driver used actual Rust-backed `ProjectService`, `MediaStore`, `NativeAudioController` and the real headless WAV callback. It advanced **256 frames**, paused, sought to **4,095**, looped `[5,8)`, stopped at frame zero and released. It then drove Chromium's real React transport via an explicitly test-only bridge backed by that controller, preparing Native twice across backend switches and observing **two releases** and **zero console errors/warnings**. Temporary service/data were closed and removed.
- Browser QA separately forced same-tick commands, idle-worker fault/retry and stale prepare/command/release failures after newer success. Fresh lead captures of web Browser, Native playing and no-device states at **1280/768/375** passed two independent visual reviews. Controls are live DOM, responsive and token-styled; timeline overflow stays in its scroller.
- Actual native device enumeration returned `{"devices":[],"v":1}`. The Electron binary/service started, but headed rendering was unavailable without an X display. Browser bridge fixtures and headless callbacks do not prove Electron IPC end-to-end execution, audible output, device latency/xruns, Windows operation or hotplug/background qualification. Native A/B/loudness matching/metering and live DSP remain unsupported rather than silently simulated.

## Bounded signal compilation (2026-10-10)

- **330 Rust tests with all features**, **97 host/API tests**, and **98 client tests** passed. Workspace all-feature build, Clippy with warnings denied, formatting, no-excuse Rust review and TypeScript/Vite build passed. Changed Rust LSP diagnostics were clean. Existing upstream Zod annotation warnings remain non-blocking.
- Coverage includes fixed typed source/filter/gain-pan/master topology, edge/cardinality/provenance validation, schedule derivation from reordered edge storage, explicit zero latency, two buffer slots, mute/solo, empty output, 256/257 active tracks, duplicate track IDs, fresh filter state and internal-versus-resource failure classification.
- Independent review initially blocked commit because instructions were built beside rather than from the graph, latency was a test-only constant, zero-frame projects no longer preserved their previous early exit and internal graph errors could map to HTTP 422. The graph now validates before schedule lowering, latency is production metadata, zero frames compile master-only before track limits, and internal compiler failures use infrastructure exit 3. Re-review approved the fixes.
- Existing block sizes 1, 3 and 4,096 remained bit-identical. The locked boundary/noise/filter WAV SHA-256, resident/file-backed equality, direct file sink, exact source-read sequence and numeric failure suites passed. The former `RenderPlan` was removed instead of retained as a parallel execution model.
- A manual CLI driver produced the same non-silent **96,044-byte** WAV twice, SHA-256 `89f3b58d17a1e41dbc786a368210c98d85a797ba40f0b31d7efe472081188017`, and rejected an unknown track. A zero-frame snapshot with **257** unused tracks and extreme finite DSP values produced exactly the expected **44-byte** WAV.
- No browser or UI behavior changed, so browser visual QA was not repeated. The graph is private and fixed: canonical routing entities, buses/sends/sidechains, feedback, PDC, automation/modulation, plugins, incremental graph caching, live native execution and physical-device qualification remain outstanding.

## Native WAV read-ahead (2026-10-10)

- **312 Rust tests with all features**, **97 host/API tests**, and **98 client tests** passed. Workspace all-feature build, Clippy with warnings denied, formatting and TypeScript/Vite build passed. Native CLI validation tests do not open devices. Existing upstream Zod annotation warnings remain non-blocking.
- New coverage includes absolute-path/hash/header/frame validation, verified-handle pathname replacement, signed PCM conversion, 4096-frame crossings, variable callback sizes, pause/resume, stop, loop normalization, one-frame loops, EOF, bounded stale draining, allocation/deallocation instrumentation, runtime truncation faults and full-queue shutdown.
- Independent review found control snapshots taken before synchronization, a missing runtime-after-readiness fault regression, and frame-limit error classification. Fixes use locked plan updates and latest-cursor CAS publication, deterministic forced interleavings, post-readiness truncation and early `TooManyFrames`. Re-review approved the implementation within the headless scope.
- An independent public-library driver used a real **12,000-frame / 48,044-byte WAV**, consumed eight frames, paused, sought to **4,095**, crossed the block boundary, exercised loop `[5,8)` after an out-of-loop seek, stopped and shut down twice. The driver initially exposed an eight-frame skip caused by retained partial-block offset; regression now checks both correct frame 4,095 and pause-without-seek resuming frame 8. Final driver passed, with four stale blocks discarded.
- Manual `daiw-play --help` and `--stream-wav ... --validate-only` succeeded with SHA-256 `fabced2ce6d34e17f35b1b2714bda9315e2af9893730d541b3762f4e0620df65`; an incorrect checksum exited nonzero. Descriptor dimensions matched 48-kHz stereo PCM16. No physical device was opened.
- LSP checks passed for the streaming modules. The optional CPAL module was unlinked in the default LSP configuration, so all-feature compiler/Clippy/tests supplied that gate instead; no clean optional-module LSP result is claimed.
- Payload ownership is bounded to eight ring blocks plus one worker and one callback block: **320 KiB decoded sample payload**, plus **16 KiB** raw PCM scratch and metadata/runtime/thread stacks. This is not a process-RSS or device-latency measurement. Device errors are counted atomically; callback code has no file I/O, locks or formatting.
- Actual Windows/Linux device opening, audible output, xrun/latency matrices, Electron-to-native control and live DSP remain outstanding. Browser behavior was unchanged, so browser QA was not rerun in this batch.

## Bounded browser playback (2026-10-09)

- **97 host/API** and **98 client tests** passed with strict TypeScript/Vite build. Relevant TypeScript/MJS LSP diagnostics were clean. Rust did not change and was not rerun; its previous 276-test result remains historical. Existing upstream Zod annotation warnings remain non-blocking.
- Host regressions cover render-info identity/RMS, exact 206 bytes and headers, invalid/oversized 416 ranges, stale keys/revisions/proposals, empty output, odd PCM scan boundaries, one full scan followed by leased chunk-only reads, file replacement 409, and lease isolation. Same-handle FD/abort checks remain green.
- Client regressions cover first-start lead, adjacency, EOF/zero output, one-frame/collapsed loops, pinned loop assembly, mapped seeks, stale generations, aggregate 16-buffer reservations, silent RMS, deferred preparation versus stop/pause/seek, and source/gain boundary automation. No production `decodeAudioData()` path remains.
- Independent review identified preparation cancelling live refills, silent transition gaps, EOF/loop edge cases and whole-file verification per range. These were corrected and re-reviewed with no remaining blocker under the documented local file-mutation trust model. Oversized modules introduced during implementation were extracted without changing APIs or assertions; both suites remained green afterward.
- Actual Chromium played a **175-second fixture for 20 seconds**, switching B/A/B at 5/10/15 seconds and then seeking near EOF. All **108** observed WAV requests were ranged 206 responses with render keys. Active unique buffers peaked at **11/16**; no unexpected alerts, console warnings/errors or page errors occurred. This observes scheduling, not physical audibility or browser-process RSS.
- Deferred A/B preparation left the existing playback running. Old sources stayed connected until a future stop boundary; replacement source and gain automation used that same time with an advanced logical offset. A six-second range stall paused at missing frame **48,000**, remained paused after release, and resumed from that frame only on explicit Play.
- Lead independently drove playback, next-bar seek and pause through real Chromium: **14** range responses, all 206 with `renderKey`, maximum **96,000 bytes**, no page errors. There were no UI styling changes; physical gaplessness, suspended-tab resilience, native devices and extended-duration policy remain unqualified.

## Bounded WAV output and HTTP streaming (2026-10-09)

- **276 Rust**, **87 host/API**, **71 client tests** passed. Rust fmt, all-feature workspace/all-target Clippy, worker rebuild and strict TypeScript/Vite build passed; relevant Rust/MJS diagnostics were clean. Existing Zod annotation warnings remain non-blocking.
- A legacy pre-refactor multi-track/noise/filter/4096-boundary golden retains SHA-256 `2a5be862a67a18c711b59d12329955ccfa442d0e39780be5287f9acd8b7cc9aa`. Tests cover block-size parity, persistent filters, absolute voice/region offsets, fractional source rounding, zero/max output, existing-file refusal, numeric failure cleanup and descriptor stdout failure.
- Independent review and integration checks led to fixes for frame-by-event traversal, source-cache thrashing, initial root containment, SIGTERM-resistant cancellation, late-handle disconnect, post-commit cleanup failure, transactional metadata invalidation and descriptor flushing. Request-owned jobs cancel; shared source regeneration continues while an aborted caller stops waiting. This deliberate boundary is tested with a paused decoder and peer request.
- Manual HTTP verification streamed **13,165,760 bytes** in **202 observed chunks** and matched the compatibility resident renderer: SHA-256 `0bd7ad00b593aaa03604cb5355b3ca851aa6bf735a7f6d21fe245bc0d83875d2`. Warm cache and repaired same-length corrupted cache returned the same digest/ETag. Cancelling delivery left no render temporaries; the next download succeeded.
- Sampled `/proc` high-water RSS for six-track silent file rendering was **5,408 KiB** at 4 seconds and **5,372 KiB** at 180 seconds. The long case wrote **34,560,044 bytes**, with only **178 descriptor bytes** on stdout. Sampling interval was 10 ms; these fixture measurements are not a universal peak-memory guarantee or CPU benchmark. Disposable data was removed.
- Host coverage includes strict header/hash verification, same-handle delivery, backpressure, pre/post-header failures, infrastructure/resource status mapping, 2 GiB reservation, restart/orphan/temp recovery, SQLite rollback and handle cleanup. Some fault-injection tests use fixtures; manual streaming and parity checks use actual binaries.
- No UI changed, so visual QA was not repeated. Browser audition remains resident and the three-minute policy remains. Long-session, real-time read-ahead, physical-device and Windows certification are not claimed.

## Offline file-backed sources (2026-10-09)

- **266 Rust**, **68 host/API**, **71 client tests** passed; worker rebuild, Rust fmt, all-feature workspace/all-target Clippy and TypeScript/Vite production build passed. Changed Rust/MJS diagnostics were clean. Existing Zod build annotation warnings remain non-blocking.
- Rust coverage includes legacy/file-backed sample and WAV parity, source offsets, block boundaries, multichannel selection, resampling, malformed headers/length/checksums/nonfinite samples, strict descriptor fields, real core snapshot metadata compatibility, large decode and typed resource exits.
- Independent review caught changed half-frame offset behavior, orphan-file budget recovery and decoder resource errors returning 422. The original half-frame test was restored unchanged, legacy checked sampling arithmetic reinstated, and an independent fractional-offset PCM golden added. Orphan recovery and real 180-second-plus HTTP 413 cases now pass.
- A manual real HTTP run imported **4,000,001 frames / 8,000,046 upload bytes**, rendered non-silent audio, altered same-length cached PCM, and regenerated byte-identical WAV with the same ETag. Corrupting the original then produced `CorruptAsset`. WAV: **16,000,116 bytes**, SHA-256 `1da8b4e89cd36cd93ceadfb50b478d9038a743afc80122ecdd7f843098a1bd4b`.
- Decoder subprocess sampling at 10-ms intervals read `/proc` high-water RSS: **5,732 KiB** for 48,000 frames; **5,668 KiB** for 4,000,001 frames. Large PCM output was **16,000,084 bytes**, but stdout only **1,591 bytes**. This supports bounded decoding for this synthetic fixture, not universal codec/RSS certification. GNU time was unavailable from configured package sources, so this is explicitly sampled measurement.
- Host tests additionally cover malformed rows, concurrent rebuild serialization, canonical metadata checks, root containment, corrupt original rejection, cache deletion/restart, near-budget orphan recovery and additive SQLite cache metadata. Some lifecycle tests use a deterministic fake decoder; the manual HTTP run and Rust/real-HTTP resource tests exercise the actual binaries.
- UI unchanged; browser visual checks were not rerun. Final output, HTTP response and browser PCM remain resident; real-time read-ahead, full-session streaming, physical-device and Windows qualification are not claimed.

## Incremental instance patterns (2026-10-09)

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
