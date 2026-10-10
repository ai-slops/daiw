# Bounded browser PCM playback

Browser audition now fetches verified PCM16 intervals and schedules small Web Audio buffers. It no longer downloads a complete WAV or calls `decodeAudioData()` to prepare playback. Full-file master downloads remain available as a separate HTTP behavior.

## Metadata and ranged transport

`GET /api/projects/:id/render-info?expectedRevision=REV` accepts optional `proposalId` and requires the expected revision. It returns:

```json
{
  "format": "daiw-wav-pcm16-v1",
  "revision": "REV",
  "renderKey": "<64 lowercase hex>",
  "sampleRate": 48000,
  "channels": 2,
  "frames": 48000,
  "byteLength": 192044,
  "dataOffset": 44,
  "bytesPerFrame": 4,
  "sha256": "<64 lowercase hex>",
  "pcmRms": 0.1,
  "pcmPeak": 0.5
}
```

Metadata and audio responses include quoted-key `ETag`, `X-DAIW-Revision`, `X-DAIW-SHA256` and `Accept-Ranges: bytes`. RMS and peak are measured over the verified signed PCM16 payload divided by 32768; empty output has zero values. Analysis happens in the bounded verification pass, not from a fabricated preview.

For frame interval `[f0,f1)`, request `render.wav` with `expectedRevision`, `renderKey`, optional `proposalId`, and `Range: bytes=44+4*f0-44+4*f1-1` (substitute the calculated integer endpoints). Only one closed, nonempty, frame-aligned range of at most **24,000 frames / 96,000 bytes** is accepted. Header, suffix, open-ended, multiple, unaligned and out-of-file ranges return 416 with `Content-Range: bytes */TOTAL`.

A successful interval is exactly 206 with matching Content-Range, Content-Length and identity headers. Revision/key/proposal mismatches return 409; missing required identity returns 400. There is no `If-Range` full-body fallback. A request without Range retains the existing full 200 download. Every path closes its file handle, including metadata responses and rejected ranges.

### Verified render leases

To avoid hashing a complete render for every half-second interval, the host keeps at most **16** verified metadata leases for **60 seconds** with LRU eviction. The exact binding is project ID, proposal ID, revision and render key. A lease retains metadata and analysis only, not PCM or an open handle.

Cold metadata/render access verifies original sources and the WAV normally. Each warm range checks the current canonical project revision, opens the managed file, and checks that same handle's `dev`, `ino`, size, nanosecond modification time and change time match the verified identity. It then reads only the requested bytes. Changed/replaced/missing files invalidate the lease and return 409 `RenderChanged`; callers must prepare again. Expiry or eviction causes a cold verification. Leases never cross project/proposal identity.

This relies on the existing immutable managed-file, single-host storage contract. It is not protection against a same-user adversary modifying an already-open inode after its identity check. Full verification still runs after lease expiry; no canonical data or persisted schema is added.

## Playback and bounds

Each normal source buffer contains 24,000 stereo frames at 48 kHz, or 500 ms. The browser validates response provenance before manually decoding the bounded little-endian PCM16 payload to f32. It primes roughly one second, targets three seconds of scheduled audio and retains at most eight unique buffers/four seconds per branch.

A shared reservation budget caps the whole engine at **16 unique buffers**, including installed, preparing and retiring branches and native-loop buffers. This bounds decoded PCM to at most **3,072,000 float bytes**, plus bounded range-response bytes, Web Audio nodes and runtime overhead. One range request per branch is in flight at a time. Diagnostics from `getStreamingMetrics()` expose aggregate ownership and reservations; active-node counts alone are not proof of total retained memory. OS/browser process RSS is not hard-capped.

Successor nodes start at the predecessor's exact end time. Initial/refilled playback uses a short 50-ms scheduling lead; its logical clock is anchored to that scheduled time. Frame conversion rounds mapped beat time at 48 kHz, matching Rust rendering. A loop's end is exclusive. Ranges split at loop/end boundaries; short loops, including a one-frame loop, use native looping after any prefix. A beat range that rounds to zero frames is rejected. Natural end and zero-frame playback remain stopped without zero-duration nodes.

Live A/B transitions keep old sources connected until a scheduled boundary and start the replacement at that boundary's advanced logical frame. Gain automation shares that boundary, including rapid switching. Loudness matching uses measured A/B RMS with the existing 0.25–4 clamp and unity if either branch is silent. Gain feeds the existing analyser, so metering remains post-gain.

## Generations, interruption and failure

Preparation cancellation is separate from transport cancellation: a slow A/B preparation does not abort the current playback's refill. Both new branches are installed atomically only when prepared. Later stop, pause, seek, loop or audition intent prevents stale preparation from overriding the user's action. Superseded fetches, buffers and callbacks are discarded; disposal stops sources, cancels preparation/refill and releases owned buffers/context resources.

If the last scheduled chunk ends without its required successor, playback pauses at the first missing render frame and emits a typed `Underrun` fault through the hook's existing error display. Late data does not auto-resume or skip forward. Explicit Play primes and resumes from that frame. Genuine network/identity failures remain visible; expected generation aborts are not user errors. `AudioContext.resume()` occurs under the user gesture before awaiting preparation.

Main-thread suspension or a network stall longer than the scheduled horizon can therefore pause playback. This is deliberate failure behavior, not the old fully resident engine's background resilience. Physical gaplessness, background/mobile policy and device latency are not certified by headless scheduling tests.

## Evidence and remaining work

Tests cover exact range identities, RMS on odd scan boundaries, lease isolation/file changes, buffer budgets, scheduling adjacency, mapped seek/loop, EOF, deferred preparations, future gain transitions, stale completions and underrun/resume. Host tests prove one full verification for metadata followed by chunk-only reads.

Real Chromium tests played a 175-second fixture for 20 seconds with A/B switches at 5/10/15 seconds, then sought near EOF and stopped normally. All 108 observed WAV requests were ranged 206 responses with render keys; no unexpected console/page errors or alerts occurred. Active unique buffers peaked at 11, while unit tests enforce the all-owned 16-buffer budget. A separate six-second delayed-range scenario paused at frame 48,000, stayed paused after release, and resumed only on explicit Play. These are network/state/scheduling observations, not a claim of audible quality.

Native verified-file read-ahead is now implemented separately in [native-streaming.md](native-streaming.md). AudioWorklet-level starvation detection, signal graph/DSP compilation, longer-duration policy, broad browser/background qualification and physical-device tests remain open. The host's three-minute render limit is unchanged.
