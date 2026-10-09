# Bounded render output and HTTP delivery

The host renders directly to a verified WAV file and streams that file to HTTP clients with backpressure. The production render/download path no longer collects a whole master, whole track scratch, encoded WAV, or host response buffer. The subsequent [browser streaming layer](browser-streaming.md) also replaces whole-WAV audition with bounded PCM ranges. This remains offline rendering, not live DSP or unrestricted long-session playback.

## DSP and WAV compatibility

The shared renderer processes 4,096 output frames at a time with one stereo `f32` master block and one reusable stereo track block. Per-track two-channel `f64` filter state persists across blocks. Notes and regions are visited in their original order, clipped to each block's overlap, and use absolute note/region-relative offsets for oscillator phase, envelope and deterministic noise. Filtering, gain/pan, summation and PCM16 quantization retain their existing expressions.

This avoids scanning every event for every output frame and avoids alternating distant same-asset source reads sample by sample. Event traversal is once per block plus actual overlapping event frames; it is not yet a fully indexed signal scheduler.

The existing resident `render()`/`wav()` APIs and stdout CLI modes remain available. Resident rendering collects blocks for callers explicitly requesting a `RenderedAudio`; the host uses the file sink instead. A pre-refactor multi-track/noise/filter/boundary SHA-256 golden and fractional-source-offset PCM golden lock successful output compatibility. Block traversal can change which of several invalid inputs is diagnosed first, but invalid audio is never published as successful partial output.

## File-output CLI

```text
daiw-render --render-to-file --source-root ABS_CACHE_ROOT --output ABS_NEW_TEMP
```

Stdin uses the existing strict `{snapshot,sources}` file-backed envelope. Output is created with `create_new`, so an existing destination is not overwritten. The sink writes the exact 44-byte WAV header, quantizes/hashes each block, flushes and syncs, and then returns a newline-terminated descriptor on stdout:

```json
{
  "format": "daiw-wav-pcm16-v1",
  "sampleRate": 48000,
  "channels": 2,
  "frames": 0,
  "byteLength": 44,
  "sha256": "<64 lowercase hex characters>"
}
```

Frames range from zero through 8,640,000; exact bytes are `44 + frames × 4`, at most 34,560,044. Descriptor stdout is capped by the host at 16 KiB. Exit 2 reports resource policy, exit 3 reports I/O/infrastructure failure, and exit 1 reports other failures. Legacy modes retain their prior exit behavior. A failed render or descriptor write/flush removes the newly owned output; a failed `create_new` never deletes someone else's existing file.

## Host publication and integrity

`MediaStore.render()` returns an open verified file handle plus descriptor, revision and render key. There is no production `.wav` Buffer compatibility property. Direct test consumers drain and close handles using test-only helpers.

The additive, rebuildable `render_cache_files` table records format, dimensions, byte length and whole-file SHA-256. Existing `renders` records retain provenance. A cold build writes under `cache/render-temp`, verifies header/size/hash, syncs, atomically renames and syncs the parent, then commits both database records in one transaction. File rename and SQLite commit are separate resources, not one atomic transaction; recovery handles the intermediate states.

Warm reads verify exact PCM16 stereo/48-kHz header layout, payload length, no trailing bytes and whole-file checksum through the same open handle later used for HTTP. Missing/corrupt metadata or files are invalidated transactionally and regenerated from verified sources. Startup removes owned temporary filenames and known unindexed WAV files, not arbitrary directory contents. Canonical root containment is checked before cleanup. Publication transfers temporary ownership at rename, so post-commit temporary-path cleanup cannot turn a successful result into failure or leak its handle.

The render cache has an independent **2 GiB** hard policy and reserves **34,560,044 bytes** before each build. Decoded sources retain their separate 4 GiB policy. There is no automatic LRU eviction yet; conservative reservation may reject a small build while some space remains. Use one host per data directory.

## HTTP and cancellation

`GET /api/projects/:id/render.wav` keeps its content type, exact Content-Length, `X-DAIW-Revision`, quoted render-key ETag and filename. The host renders and verifies the file before sending headers, then uses a backpressured file-to-response pipeline and closes the handle in `finally`. No range or progressive-before-render-completion API is added. Stale revisions remain 409, resource limits 413, invalid audio 422, and output infrastructure failures 500.

After headers, stream failure destroys the response rather than appending JSON to WAV bytes. Client disconnect aborts request-owned rendering and delivery; a late returned handle is closed without starting a stream. Already-aborted queued work is skipped. Owned subprocess cancellation sends SIGTERM, escalates to SIGKILL after 500 ms if necessary, and waits for child closure before deleting temporary output.

Decoded-source regeneration is shared cache work, not request-owned. An aborted render stops waiting promptly while that operation may finish and atomically publish a valid source cache for another consumer. It does not publish a partial render or keep the disconnected request waiting for the decoder. Completed valid cache files may survive a disconnect.

## Bounds and observed evidence

Output scratch is two 4,096-frame stereo f32 blocks (64 KiB combined), plus a 16-KiB PCM16 encoding block. Prepared event/track state, source read buffers, input JSON, checksumming, allocator overhead and process runtime are additional. No total-process hard memory cap is claimed.

A live HTTP fixture download was 13,165,760 bytes, delivered in 202 observed client chunks, and matched the resident compatibility renderer's SHA-256. Chunk count is transport-dependent, not a contract. Same-length WAV corruption rebuilt identically; cancelling a download left no render temporaries and the next request succeeded.

For six-track silent output fixtures, sampled Linux `/proc` high-water RSS was 5,408 KiB for four seconds and 5,372 KiB for 180 seconds. The latter produced a 34,560,044-byte WAV while stdout was only 178 bytes. These 10-ms samples demonstrate the tested output path, not a universal memory bound or performance certification. See [verification.md](verification.md) for hashes, counts and test scope.

Bounded browser streaming playback is now implemented separately. Remaining work includes native streaming playback, real-time read-ahead, longer-duration policy qualification, signal graph/DSP compilation and physical-device testing. The three-minute output policy remains unchanged.
