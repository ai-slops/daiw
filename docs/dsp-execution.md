# Prepared DSP execution boundary

`daiw-audio` exposes an owned compiled program and a persistent variable-block executor. This is a library execution boundary, not device playback, program hot swap, a live file-source service or incremental compilation. Native/Electron physical qualification is explicitly deferred until an appropriate environment is available.

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

## Evidence and next boundaries

Final gates passed **356 all-feature Rust tests**, **122 host tests**, **106 client tests**, workspace all-feature build/Clippy, formatting and TypeScript/Vite build. Allocator instrumentation checks success, EOF, first late fault, repeated fault and reset replay with zero allocations and deallocations. Existing file-source parity, duplicate-track, read-order and pre-refactor WAV golden tests pass.

An independent external crate imported the public API, compiled from a temporary snapshot, processed **7,001 frames** with request sizes **1, 3, 4,097 and 5,000**, and compared the result bit-for-bit with offline rendering of synthesis plus a resampled 32-kHz stereo source. EOF tail and reset replay matched. A second fixture faulted at frame 4,096 of a 4,097-frame call, zeroed the complete output, retained cursor 4,096 and returned the same error on repeat and reset replay.

Still required: callback-boundary publication/retirement and last-valid-program ownership, disk-source prefetch, time-indexed active events for larger sessions, incremental compilation, transport seek/loop state policy and integration with a device callback. General routing, automation, PDC and plugins remain outside the fixed graph. No UI/device behavior was changed and no physical/native qualification is claimed.
