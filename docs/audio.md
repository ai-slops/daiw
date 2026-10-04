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

Output is deterministic 48 kHz, stereo, signed 16-bit little-endian PCM. Synth, bass, kick, snare, hat, and pad notes use deterministic built-in voices. Each track applies mute/solo, dB gain, constant-power pan, and a one-pole low-pass. Sample regions preserve stereo, support mono or interleaved PCM, sample-rate conversion, beat placement, positive duration, dB region gain from -96 through +24 (`0` is unity), and source offsets in seconds. Missing assets, unknown track references, duplicate effective note IDs, invalid checked numeric inputs, and malformed interleaved PCM are explicit render errors.

The core now validates emitted note identity, pitch/velocity and timing after modifiers, repetition and crop, including a global ID check across instances. HTTP rendering obtains that validated snapshot before looking up a render cache. The standalone renderer additionally rejects duplicate IDs at its own input boundary; existing NaN/Infinity rejection remains in place. Ordinary fractional pitch such as `60.25` is covered by a finite, non-silent render regression.

Input/data validation and DSP validation are separate. The renderer now rejects unrepresentable frequency/phase, non-finite gain conversion, non-finite rendered track samples and non-finite master samples. The public WAV encoder independently rejects NaN/Infinity before allocation and quantization. Invalid values are not silently replaced with zero, silence or full-scale PCM. Ordinary finite samples still clip to `[-1,1]` at PCM encoding, preserving prior quantization.

### Numeric and output-memory boundaries

- `Synth`, `Bass` and `Pad` require a finite positive pitch-derived frequency and finite phase at the last actually rendered sample. No arbitrary integer-MIDI or Nyquist-range clamp is introduced; this is numeric validity, not an anti-aliasing guarantee.
- Muted/non-solo tracks, empty/clipped-out notes and placeholder `Sample` notes do not evaluate unused pitch arithmetic. Kick/snare/hat retain their pitch-independent synthesis. Global input validation still rejects malformed/non-finite input fields even on unused data.
- Clipped notes retain their nominal rounded duration as the envelope denominator. Existing beat-to-frame and source-offset rounding order is preserved. Derived frame indices must be finite, nonnegative and representable rather than relying on saturating float casts.
- Source resampling uses checked integer arithmetic. Representable offsets past the source remain ordinary source-end silence; unrepresentable offsets fail. Region multiplication, filter output and master accumulation cannot publish non-finite samples.
- Finite linear-gain underflow to zero remains silence. Overflowing linear or pan gains fail explicitly. Finite internal headroom above unity is allowed; the WAV encoder clips it as before.
- `MAX_RENDER_FRAMES` is **8,640,000 stereo frames** for the Rust renderer, checked before allocating output buffers. This is 180 seconds at 48 kHz or 90 seconds at 96 kHz. The existing HTTP endpoint remains fixed at 48 kHz and separately limits duration to 180 seconds.
- Master and a reused scratch track buffer are allocated fallibly. At the cap they request about **138.24 MB total** for two stereo `f32` buffers; later master-plus-WAV storage is about 103.68 MB. These numbers exclude assets, input JSON, host/cache buffers and allocator overhead, and are not a total process-memory or CPU guarantee.
- Rust reports `InvalidFrameCount`, `RenderTooLarge`, `Allocation`, or `InvalidValue` with fields such as `frequency`, `phase`, `linearGain`, `panGain`, and `samples`. `WavTooLarge` continues to protect WAV header/payload arithmetic. Allocation failures are not mislabeled as numeric failures.

All checks run in offline/control-thread rendering or encoding, not in the audio callback. A successful render returns finite samples, and a failed render does not return partial audio. Restart the host after deploying a new renderer so `MediaStore.initialize` refreshes its binary fingerprint; render cache keys include that fingerprint. The host writes a WAV/cache record only after successful worker completion.

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
