# Audio subsystem

`daiw-audio` consumes the immutable `Snapshot` wire shape in `docs/contract.md`. It does not depend on `daiw-core`, so the renderer can remain a stable process boundary while core evolves.

## Offline renderer

`daiw-render` reads one JSON value from stdin and writes only PCM WAV bytes to stdout. Diagnostics and failures use stderr.

```sh
cat snapshot.json | cargo run -p daiw-audio --bin daiw-render > mix.wav
cat render-envelope.json | cargo run -p daiw-audio --bin daiw-render > mix.wav
cargo run -p daiw-audio --bin daiw-render -- --help
```

The accepted values are a bare `Snapshot`, the host's flattened `{...Snapshot, assets}` value, or:

```json
{
  "snapshot": { "revision": "…", "tempo": 120, "length": 8, "tracks": [], "notes": [], "regions": [] },
  "assets": {
    "asset-id": { "sampleRate": 48000, "channels": 2, "samples": [0.0, 0.0] }
  }
}
```

Output is deterministic 48 kHz, stereo, signed 16-bit little-endian PCM. Synth, bass, kick, snare, hat, and pad notes use deterministic built-in voices. Each track applies mute/solo, dB gain, constant-power pan, and a one-pole low-pass. Sample regions preserve stereo, support mono or interleaved PCM, sample-rate conversion, beat placement, positive duration, dB region gain from -96 through +24 (`0` is unity), and source offsets in seconds. Missing assets, unknown track references, non-finite or out-of-range values, and malformed interleaved PCM are explicit render errors.

The host can decode WAV, AIFF, or FLAC uploads into its content-addressed PCM cache without a second binary:

```sh
cargo run -p daiw-audio --bin daiw-render -- --decode /absolute/path/to/upload.flac
```

The JSON result is `{frames,sampleRate,channels,samples}` with interleaved `f32` PCM. Decoding is capped at 4,000,000 interleaved samples per source. The decoder checks each decoded packet's frame/channel capacity before allocating its conversion buffer or extending accumulated PCM and returns typed `ResourceLimit` on hostile compressed expansion.

## Browser API

Import from `src/audio`:

```ts
const audio = new DaiwAudioEngine();
audio.setSnapshot(snapshot);
await audio.prepare(projectId);
audio.setLoop({ start: 4, end: 8 });
audio.seek(4);
await audio.play();
const unsubscribe = audio.subscribe((state) => updateTransport(state));

await audio.prepareAB(projectId, proposalId);
audio.setLoudnessMatching(true);
audio.selectAB("b");
```

`setSnapshot` parses untrusted data with Zod. `prepare` must decode the canonical server WAV before playback; the browser has no divergent local synth fallback. Every render GET includes `expectedRevision` captured from the current immutable snapshot and must return the same `X-DAIW-Revision`. After network and decode awaits, the engine rechecks that the active snapshot still has the captured revision before installing buffers. A missing/mismatched header or concurrent snapshot edit rejects and discards the stale result; the two concurrent A/B GETs share one captured revision. The transport API is `play`, `pause`, `stop`, `seek`, `setLoop`, `getPositionBeats`, `getState`, and `subscribe`. Calls to `prepareAB` decode the canonical server WAV for the current snapshot and proposal concurrently; `selectAB` preserves the exact beat offset while switching immutable buffers. `setLoudnessMatching` optionally applies a bounded measured-RMS gain to B against A. Buffer playback uses Web Audio offsets and end-exclusive loop points, loops cannot exceed snapshot length, and natural source completion updates state to stopped at the snapshot end.

`getMeter()` returns `{peak,rms,peakDb,rmsDb}` from the actual post-gain master signal. A persistent `AnalyserNode` and preallocated 2,048-sample `Float32Array` make it suitable for UI `requestAnimationFrame` reads. Stopped/paused silence is `{peak:0,rms:0,peakDb:-Infinity,rmsDb:-Infinity}`.

Browsers require `play()` to originate from a user gesture before `AudioContext.resume()` can succeed.

## Native output

The optional `native` feature uses CPAL:

```sh
cargo run -p daiw-audio --features native --bin daiw-play -- snapshot.json
cargo run -p daiw-audio --features native --bin daiw-play -- --help
```

The control thread renders and owns immutable interleaved buffers. `replace_audio` enqueues one replacement through a capacity-one preallocated `rtrb` SPSC ring. The device callback adopts it only at the next callback block boundary, moves the prior `Arc` to a second preallocated retirement ring, copies samples, and advances or wraps the atomic cursor. Allocation and deallocation are forbidden in the callback and enforced by a `stats_alloc` regression test. `reclaim_replaced_audio` drops retired buffers on the control thread; `replace_audio` also reclaims before enqueueing. A busy queue returns `SwapBusy` rather than blocking. Seek requests use a pending atomic consumed at the next block boundary, so a control-thread seek arriving mid-block cannot be overwritten by the callback's cursor commit. Loop fields use a generation-guarded publication; the callback makes one bounded snapshot attempt per block and disables looping for that block rather than exposing a mixed range during a concurrent update. Seek and loop offsets are interleaved-sample offsets.

CPAL selects the native Windows or Linux host. This workspace validates the callback and transport without hardware. Actual WASAPI/ALSA device opening cannot be asserted in a headless container; the CLI reports missing devices, unsupported non-f32 defaults, and format mismatches rather than silently resampling in the real-time callback. CPAL's platform error callback performs no formatting or logging; it only increments an `AtomicU64`, exposed for control-thread polling through `error_count()`. Linux native builds also require the distribution's ALSA development package.
