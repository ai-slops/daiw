# Offline file-backed audio sources

The HTTP host now decodes imported sources directly into managed binary PCM files and passes bounded descriptors, not sample arrays, to the renderer. Rendering reads each source in blocks of at most 4,096 source frames. This removes whole-source PCM JSON and source-size-proportional decoded buffers from the host/render input path.

This is **offline source streaming**, not full-session or real-time streaming. The subsequent [output-streaming layer](output-streaming.md) now bounds production render output and HTTP delivery too. Upload bodies and browser `AudioBuffer`s remain resident; legacy resident render APIs remain available. Output duration stays capped at three minutes. Native callback read-ahead, browser/native streaming playback and device qualification remain open.

## CLI and binary format

Legacy no-argument resident rendering and `--decode ABS_PATH` remain supported. Legacy decode retains its 4,000,000-interleaved-sample limit. The host uses two new modes:

```text
daiw-render --decode-file --input ABS_OBJECT --output ABS_NEW_TEMP
daiw-render --render-file-backed --source-root ABS_CACHE_ROOT
```

Decode creates a new output file, writes interleaved f32 little-endian PCM incrementally, finalizes checksums/header, flushes and syncs, then emits only `{format,frames,sampleRate,channels,byteLength,sha256,peaks}` JSON. `peaks` has 128 bins; stdout is capped by the host at 16 KiB. Failure removes the incomplete output owned by that decode attempt.

`daiw-f32le-v1` has an 80-byte header:

| Offset | Value |
|---|---|
| 0–7 | `DAIWPCM` plus zero byte |
| 8–11 | u16 version 1, u16 header length 80 |
| 12–19 | u32 sample rate, u16 channels, u16 sample format 1 |
| 20–35 | u64 frames, u64 payload byte length |
| 36–67 | SHA-256 of payload |
| 68–79 | Reserved zero bytes |

All integers are little-endian. Payload length must equal `frames × channels × 4`; file length must be exactly header plus payload. The descriptor separately hashes the complete file, including the header.

File-backed rendering reads `{snapshot,sources}` from stdin. Each source descriptor contains only `cacheId`, `sha256`, `frames`, `sampleRate`, `channels`, and `byteLength`. Cache IDs are 64 lowercase hex characters; the renderer derives `<source-root>/<cacheId>.pcm`. The outer envelope and descriptors reject unknown fields. Existing rendering-irrelevant snapshot metadata, such as meter maps and nested provenance, remains compatible.

Canonical projects contain asset identities and metadata, not filesystem paths. Only the trusted host constructs CLI paths. Host and renderer check canonical containment of managed paths; source descriptors cannot select arbitrary paths. The renderer verifies the exact referenced source set, headers, metadata, length, whole-file hash, payload hash and finite samples before rendering. It retains the same open file handles for block reads. Same-user adversarial mutation of files/database while rendering is outside this single-host storage contract.

## Sampling compatibility

Resident and file-backed sources share the sampling kernel. Source offset is rounded once to a source frame; subsequent advancement uses checked integer `floor(outputOffset × sourceRate / outputRate)`. No new interpolation is introduced. Mono duplicates to stereo; multichannel uses the first two channels. Region gain, filtering, mixing, source-end silence and PCM16 quantization remain unchanged. Fractional-offset and block-crossing regression fixtures compare exact samples/WAV bytes, including an independent expected PCM golden.

Each source retains one encoded block, at most 128 KiB for eight channels. A block is fetched with one bounded read and samples are decoded from that buffer. Initial integrity scanning uses bounded scratch space. Symphonia packet conversion is limited before allocating our conversion buffer, but codec-owned internal allocations are not a proven hard process-memory bound.

## Managed cache lifecycle

`decoded_audio_cache` is an additive, rebuildable SQLite table keyed by source hash, renderer fingerprint and PCM format version. It records deterministic cache identity, whole-file checksum, metadata and waveform peaks. None of these cache records or files are native musical project state.

Source lookup, verification, deletion and rebuild run through one serialized lifecycle queue. Every use verifies the original blob's hash and canonical asset metadata. Managed PCM checksums are compared against the database descriptor. Missing, truncated, altered, malformed or inconsistent caches are removed and rebuilt once from the verified original. A corrupt original fails explicitly as `CorruptAsset`; an intact decoded cache is not a substitute for it.

Publication uses a temporary file, sync and atomic rename before metadata insertion. Startup removes abandoned source temporaries. A published PCM with no database row is treated as a rebuildable orphan and reclaimed before reserving rebuild capacity. Waveform requests use descriptor peaks instead of loading the full PCM. Render keys include snapshot, sorted asset/checksum pairs, renderer fingerprint and file-backed profile version. Failed render workers do not publish WAVs or render rows.

Use one host per data directory. Builds and budget reservations are serialized in that host; concurrent independent hosts sharing the directory are not supported by this slice.

## Limits

| Resource | Policy |
|---|---:|
| Uploaded source file | 20 MiB |
| Source duration | ≤180 seconds |
| Sample rate / channels | 8,000–192,000 Hz / 1–8 |
| Decoded source payload | 512 MiB, plus 80-byte header |
| Our decoded packet conversion buffer | 8 MiB |
| Distinct source files per render | 64 |
| Referenced PCM file bytes per render | 1 GiB |
| Managed decoded cache + source temporaries | 4 GiB |
| Reservation per cache build | 512 MiB + 80 bytes |
| Output | Existing 8,640,000 stereo-frame cap and HTTP 180-second limit |

The conservative maximum-size reservation can reject a small new source before the disk is completely full. There is no decoded-cache LRU or automatic source garbage collection yet. Limits are policy choices, not desktop-scale benchmark results.

New CLI modes use exit code 2 for typed resource limits; the host opts into mapping that code to HTTP 413 `ResourceLimit`. Other new-mode failures remain exit 1 / HTTP 422 `AudioError` unless the host detects a more specific asset error. Legacy CLI modes retain their prior generic exit behavior.

## Evidence

An independent real HTTP run imported and rendered a 4,000,001-frame mono WAV, beyond the old host sample cap. Altering its PCM payload caused regeneration with the same render ETag and byte-identical WAV. Altering the original then returned `CorruptAsset`.

Separate decoder subprocess sampling observed `/proc` high-water RSS of 5,732 KiB for 48,000 frames and 5,668 KiB for 4,000,001 frames. The large decoded file was 16,000,084 bytes while descriptor stdout was 1,591 bytes. These are sampled Linux measurements for synthetic PCM fixtures, not an absolute peak-memory guarantee for all codecs or the full renderer. Full test and WAV evidence is in [verification.md](verification.md).
