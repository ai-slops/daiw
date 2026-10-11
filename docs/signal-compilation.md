# Bounded signal compilation

The offline renderer lowers every non-empty validated playback snapshot into a crate-private typed signal graph before processing audio blocks. The graph remains private, while its owned `CompiledDspProgram` now also supports a prepared resident-source executor. This makes topology, execution order, buffer ownership, latency and provenance explicit without adding a canonical project graph or changing the snapshot wire format. See [dsp-execution.md](dsp-execution.md) for the public execution boundary.

## Fixed graph contract

Each active track compiles to one stereo path:

```text
TrackSource -> OnePoleLowPass -> GainPan -> Master[input = active track ordinal]
```

Mute and solo filtering happens before graph construction, exactly as in the previous renderer. Notes and regions are ordered payload inside `TrackSource`, not graph nodes. The master is unique and has no outgoing edge. Source nodes have no input; filter and gain/pan nodes each have one input and one output. Every node must reach the master through the fixed path.

Node IDs are dense compile-local ordinals. They are not canonical entity IDs. Provenance records the snapshot revision and, for every track node, the active track ordinal, original track ID and source/filter/gain-pan stage. The ordinal is required because duplicate track IDs retain the legacy behavior of producing distinct audible paths that each consume matching events.

All ports are typed stereo audio ports. Mono and multichannel region selection remains an event/source concern inside `TrackSource`. There are no event/control ports, user graph builder or persisted routing records in this slice.

The compiler rejects dangling or duplicate edges, mismatched node/provenance kinds, sparse or reordered master ports, incomplete paths, invalid cardinality, unsupported latency and resource-limit violations. A non-empty snapshot supports at most **256 active tracks**, therefore at most **769 nodes**, **768 edges** and **1,282 instructions**. Existing effective-event, source-file and 8,640,000-output-frame limits remain separate and unchanged.

Zero-duration snapshots preserve the prior empty-render contract: they compile directly to a master-only program before track counting or DSP preparation. Consequently, otherwise valid unused tracks cannot trigger the active-track limit or derived DSP arithmetic for a zero-frame WAV.

## Graph-derived execution

The graph is built and validated before its execution schedule exists. Schedule compilation sorts master edges by their input ports, follows each validated gain-pan to filter to source path, reads the path's provenance ordinal and emits:

1. Clear master once per block.
2. Clear the reusable track buffer.
3. Render that track's notes, then regions.
4. Apply its stateful one-pole filter.
5. Validate finite track samples.
6. Apply gain/pan and accumulate into master.
7. Validate finite master samples after all paths.

Execution consumes only this derived instruction vector. Reordering edge storage therefore cannot reorder track accumulation. The schedule retains the previous block to track to region to source-frame access sequence, floating-point expressions and master summation order.

The buffer plan has exactly two slots: one 4,096-frame stereo master block and one reusable stereo track block, **64 KiB combined**. A fresh execution state owns two `f64` filter values per active track. Prepared events, source caches, the sink conversion block and runtime/allocator storage are additional. Resident rendering still allocates its requested final output; direct file rendering remains bounded.

Every current node exposes production `latency_frames = 0`. Program latency is compiled from node metadata and checked again by execution. Any nonzero latency is rejected rather than ignored; delay nodes and compensation do not exist yet.

## Compatibility and errors

Public `render`, file-backed rendering, direct WAV output and existing CLIs retain their interfaces. The former `RenderPlan` is removed rather than kept as an alternate execution model. Source metadata/read and `BlockSink` remain the media boundaries.

The compiler reports a user-facing `TooManyActiveTracks` resource error for more than 256 active paths. `InvalidSignalGraph` and internal graph resource overflows are compiler/infrastructure failures; `--render-to-file` maps them to exit 3 rather than classifying them as invalid project audio. No partially compiled or partially rendered result is published after failure.

## Verified scope

Regression coverage inspects topology, typed endpoints, provenance, graph-derived instruction order, edge-storage reordering, zero latency, zero-frame lowering, 256/257-track boundaries, duplicate track IDs, malformed private graphs and fresh execution state. Existing block sizes 1, 3 and 4,096 remain bit-identical. Resident/file-backed parity, exact source read order, fractional source offsets, numeric failures and the pre-refactor WAV SHA-256 golden remain green.

An independent CLI driver rendered the same non-silent one-second snapshot twice to identical **96,044-byte** WAVs with SHA-256 `89f3b58d17a1e41dbc786a368210c98d85a797ba40f0b31d7efe472081188017`, rejected an unknown track reference, and rendered a zero-frame snapshot containing 257 unused extreme-valued tracks to the expected **44-byte** WAV.

This does **not** add canonical signal entities, persistence/migrations, graph editing, buses, auxes, sends, sidechains, parallel fan-out, feedback, delay nodes, PDC, automation/modulation ports, per-note graph nodes, plugins, incremental graph caching, hot swap or native callback execution. Physical-device behavior is unchanged and was not exercised. Those features require separate contracts rather than widening this fixed compiler implicitly.
