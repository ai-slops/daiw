# Prepared DSP execution boundary

`daiw-audio` exposes an owned compiled program, a persistent variable-block executor and bounded callback-boundary publication with deferred retirement. This is a library execution boundary, not device playback, a live file-source service or incremental compilation. Native/Electron physical qualification is explicitly deferred until an appropriate environment is available.

## Control-time preparation and process-time execution

`CompiledDspProgram::compile(&Snapshot, &impl SourceMetadataProvider, sample_rate)` validates the snapshot and lowers the existing fixed stereo source/filter/gain-pan/master graph. It owns runtime scalars, instructions, revision, diagnostic IDs and source metadata. It does not retain the snapshot or copy PCM samples; the snapshot may be dropped after compilation.

`ResidentDspSources::new(&assets)` wraps an immutable resident asset map. `DspExecutor::prepare(program, sources)` owns the program, resolves each source to a direct immutable asset reference, validates its metadata, 1–8 channels, divisible sample layout and finite samples, and allocates all filter/scratch state. Compilation and preparation are control-thread work.

```rust
use daiw_audio::{CompiledDspProgram, DspExecutor, ResidentDspSources};

// `snapshot` and `assets` are already constructed application inputs.
let sources = ResidentDspSources::new(&assets);
let program = CompiledDspProgram::compile(&snapshot, &sources, 48_000)?;
let mut executor = DspExecutor::prepare(program, sources)?;
let mut output = [[0.0_f32; 2]; 256];
let report = executor.process(&mut output)?;
// report.frames counts rendered stereo frames, excluding the zero-filled EOF tail.
```

The public getters are `sample_rate`, `total_frames` and `revision` on the program, and `cursor`, `state` and `program` on the executor. The PCM owner must outlive the executor, and Rust's immutable borrow prevents mutation or deletion while those sources are in use.

`process(&mut [[f32; 2]])` and `reset()` perform no allocation/deallocation, map lookup, file I/O, locks, logging or string formatting. Runtime faults are numeric, `Copy` `DspProcessError` variants; offline error conversion and formatting happen outside the shared kernel. Dropping an executor/program can deallocate and therefore belongs off the audio callback.

## Variable-block and state contract

- Any caller-buffer length is accepted. Requests longer than 4,096 frames are processed in bounded internal chunks, with persistent filter state and cursor across chunks and calls.
- Empty buffers report zero frames without advancing. Empty programs are `Ended` immediately after preparation and reset.
- Successful output is bit-identical to offline rendering, including track summation, oscillator/noise offsets, region resampling and filter arithmetic.
- EOF leaves the unused caller-buffer tail zero, reports `Ended`, and preserves the final cursor on subsequent calls.
- A new fault zeroes the **entire caller buffer**, even if earlier chunks in that call succeeded. Cursor retains the last successfully committed chunk. Filter state may have partially advanced inside the failed chunk, so no recovery from that position is implied.
- Faults are sticky: later calls zero their output and return the original error. `reset()` clears fault, cursor, filter history and scratch state and restarts at frame zero.
- No seek or loop API is offered. Correctly restoring stateful filters at arbitrary transport positions requires a separate contract.

States are `DspProcessState::{Running, Ended, Faulted}`. Runtime errors distinguish unavailable source frames, frame-index overflow, non-finite track output by track ordinal and non-finite master output. A failed call never publishes partial successful audio.

## Limits, source descriptions and offline reuse

The public compiler admits at most **4,096 prepared note/region instances**, counted after mute/solo filtering, clipping, placeholder sample-note removal and duplicate-track expansion. It checks the limit before building the event arrays. A compile-scoped cache reads each successful asset metadata description once and shares it between counting and construction, so a stateful provider cannot change the interpretation between passes. The constructed event count is checked against the pre-count.

The existing 256-active-track and 8,640,000-output-frame policies remain. Zero-output snapshots still compile directly to a master-only program before unused-track/event limits. Two 4,096-frame stereo work buffers account for 64 KiB of sample scratch, with filter/source bindings and program metadata additional. This is not a total memory or hard real-time deadline guarantee: arbitrary caller lengths and up to 4,096 event scans still consume proportional work.

The private offline compiler does not impose the new callback-oriented event cap. Existing offline resident/file-backed/streamed WAV entrypoints use the **same `execute_span` arithmetic kernel**, preserving source reads and error conversion at their original execution points. `FileSourceBank` can seek/read/resize its cache and cannot be supplied to the public executor's resident-only capability.

For Rust source-provider implementors, metadata now belongs to `SourceMetadataProvider`; `SourceBank` extends that trait with frame reads. Existing custom banks must move their `metadata` implementation into a separate `impl SourceMetadataProvider`, retaining `read_frame` in `impl SourceBank`. CLI, snapshot and render wire formats are unchanged.

## Bounded publication and retirement

`DspSwap::channel(initial_executor)` returns `DspSwapControl` and `DspSwapCallback`. Construction resets the initial executor and allocates two capacity-one SPSC `rtrb` queues off callback. The channel fixes its sample rate from the initial program. Each endpoint has a single owner; borrowed resident PCM can span `thread::scope` while remaining immutable.

The control side exposes:

- `enqueue(prepared_executor)`: checks sample rate and observed callback abandonment, resets the candidate off callback, then publishes it to the pending slot. Success means **queued**, not activated or guaranteed receiver liveness; concurrent endpoint destruction can race with the abandonment check.
- `compile_and_enqueue(&snapshot, resident_sources)`: compiles and prepares at the channel sample rate, then enqueues. Both build stages map to `DspPublicationError::Build(AudioError)`; publication can instead return `SampleRateMismatch { expected, actual }`, `QueueFull` or observed `Disconnected`. Build failures take precedence over transient full/disconnect errors because there is no capacity preflight. Every failure preserves the active and existing pending executors; a rejected candidate is dropped by the control API caller.
- `reclaim()`: explicitly drops available retired executors on its caller thread and returns their count. It is not automatically invoked by enqueue.
- `shutdown(matching_callback)`: after the host has stopped processing and returned the callback endpoint, drops both endpoints on its caller thread. The matching-half requirement is not checked dynamically.

At the beginning of `DspSwapCallback::process`, the callback retries one deferred retirement. Only when that slot is empty can it take one pending executor. It replaces the whole current executor, moves the previous executor to the retired queue or back to the deferred slot on saturation, then processes the entire caller buffer once with the current executor. Even a buffer longer than 4,096 cannot change programs between its internal chunks. An empty buffer is still a publication boundary.

Ownership is bounded to **four channel-held executors**: one active, one pending, one retired and one deferred. A caller-owned candidate or transient build is additional. If retirement remains blocked, subsequent replacements wait in the pending slot while the current program keeps running; another submission gets `QueueFull`. Callback processing never destroys an executor, including failed retirement pushes, and performs no allocation, locks, file I/O or formatting. Control must reclaim regularly for publication to keep advancing.

Callback `program`, `cursor` and `state` describe the active executor. `reset()` affects only the currently active executor, not queued or retired programs. Every newly activated program starts at frame zero with fresh filter/fault state. A successfully activated program that later faults remains current with the executor's sticky-fault behavior: there is no runtime-fault rollback. Reset or a later valid replacement can recover it. There is no seek continuity, crossfade, click-free transition or acknowledgment/status service on the control endpoint.

**Ordinary endpoint destruction is not real-time safe.** If control is dropped first, an already queued candidate may still activate; old owners remain bounded in the orphaned retired/deferred slots. If callback is dropped first, queued owners remain in channel storage until the final endpoint is dropped. Neither case leaks ownership, but destruction may deallocate on the dropping thread. The host must quiesce processing and move callback ownership off the audio thread before teardown. The borrowed PCM lifetime is unchanged; a future `'static` device adapter needs its own owned-source contract.

## Evidence and next boundaries

Final publication gates passed **363 all-feature Rust tests**, **122 host tests**, **106 client tests**, workspace all-feature build/Clippy, formatting and TypeScript/Vite build. Allocator instrumentation checks success, EOF, faults, reset, replacement, deferred retirement and retry with zero allocations and deallocations. Existing file-source parity, duplicate-track, read-order and pre-refactor WAV golden tests pass.

An independent external crate imported the public API, compiled from a temporary snapshot, processed **7,001 frames** with request sizes **1, 3, 4,097 and 5,000**, and compared the result bit-for-bit with offline rendering of synthesis plus a resampled 32-kHz stereo source. EOF tail and reset replay matched. A second fixture faulted at frame 4,096 of a 4,097-frame call, zeroed the complete output, retained cursor 4,096 and returned the same error on repeat and reset replay.

An additional independent public-API crate preserved existing PCM/cursor after a failed build, observed queued-versus-active revision differences and full-queue rejection, and matched replacement PCM bit-for-bit across a **4,097-frame** invocation. Retirement saturation delayed the next replacement until explicit reclaim. Per-call instrumentation measured zero callback allocations/deallocations; reclamation and final shutdown measured positive deallocations on the control caller. A scoped callback thread returned its borrowed endpoint to control before teardown. Independent final ownership review found no blocking issue.

Still required: disk-source prefetch, time-indexed active events for larger sessions, incremental compilation, transport seek/loop state policy and integration with a device callback. General routing, automation, PDC and plugins remain outside the fixed graph. No UI/device behavior was changed and no physical/native qualification is claimed.
