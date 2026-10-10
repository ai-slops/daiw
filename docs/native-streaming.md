# Native WAV read-ahead playback

`daiw-audio` now provides a CPAL-independent verified-WAV streaming library and an optional `NativeWavPlayer` adapter. A dedicated disk worker fills a fixed-capacity ring; the device callback only consumes prepared samples and updates atomics. Existing resident `NativePlayer`, `Transport`, replacement-buffer APIs and snapshot CLI modes remain available.

This plays an already rendered canonical WAV. It does not perform live synthesis, graph recompilation, recording or project editing in the callback. The Electron application still uses browser audio; native device selection/control is not wired into its UI.

## File boundary and units

`validate_wav_file(path, &WavFileDescriptor)` requires an absolute path, the renderer's exact 44-byte PCM16 stereo/48-kHz header, exact file length, and matching whole-file SHA-256. Zero-frame files are accepted; the existing 8,640,000-frame limit remains. The returned `WavStreamFile` owns the verified open file. `file.open(WavStreamOptions)` moves that same handle to the worker instead of reopening the path.

The CLI receives a mandatory expected SHA-256 and obtains dimensions from the WAV header before full validation. A wrong checksum is rejected before opening a device. Renaming/replacing the pathname after validation does not redirect playback to a different file. As with managed render files, mutation of the already-open inode after verification is outside the immutable-file contract; later read failures such as truncation are surfaced explicitly.

New controls use **stereo frame** positions through `FramePosition`, `FrameLoopRange` and `WavStreamOptions`. The older resident transport's interleaved-sample offsets are unchanged. One new frame corresponds to two old-style interleaved samples; callers must not mix these domains.

## Public library lifecycle

`WavStreamFile::open(options)` returns `WavStreamControl` and `WavStreamCallback`. Opening starts the worker paused and waits for initial bounded prefill or a typed startup failure. A native output callback calls `fill_output(&mut [f32])`; control-thread clients use:

- `play`, `pause`, `stop`, `seek(FramePosition)`, `set_loop(Option<FrameLoopRange>)`.
- `position`, `state`, `stats`, `take_error`.
- `shutdown`, which wakes and joins the worker and is idempotent.
- `device_error_reporter`, a cloneable atomic-only error counter handle for the device error callback.

State is `Paused`, `Playing`, `Ended`, `Faulted` or `Shutdown`. Terminal fault/shutdown cannot be revived by later controls. Generation exhaustion is reported rather than wrapping and risking ABA.

| Event | Behavior |
|---|---|
| Pause | Silence without advancing; valid partial-block position retained |
| Stop | Pause plus generation-changing seek to frame zero |
| Seek | New generation takes effect at a callback boundary; stale blocks discarded |
| Loop | End-exclusive frame range; short loops may repeat inside a worker block |
| Seek at/past loop end | Published cursor and worker position both normalize to loop start |
| Underrun | Unsupplied callback tail is zeroed; cursor stays at first missing frame; one underrun counter increment |
| Supply after underrun | Matching data resumes from that same frame, without skipping |
| EOF | Final partial block is consumed, tail zeroed, position reaches file end, state becomes `Ended` |
| Read failure | First typed I/O error is available to control polling; state becomes `Faulted` |
| Shutdown | Worker stops; CPAL adapter drops its device stream before joining the worker |

Unlike browser underrun policy, native supply recovery resumes automatically while domain state remains playing. No file read, allocation/deallocation, mutex, logging or formatting occurs in `fill_output`. Old-generation discard is bounded to eight blocks per callback; valid copy work is bounded by output length. Odd trailing output samples are silenced.

## Concurrency and memory

Control commands serialize complete read-plan updates under a control-only mutex. Cursor and lifecycle are generation-tagged atomics. Callback cursor/EOF commits cannot overwrite a newer seek, loop or shutdown state. Loop changes CAS-retag the latest old-generation position, preserving a callback that advanced while the command was being published. Worker read failures are likewise generation-scoped.

The ring contains eight inline blocks of 4,096 stereo f32 frames. Including one worker-local and one callback-current block, up to ten decoded blocks can be owned: **327,680 bytes (320 KiB)** of sample payload. A 16-KiB worker PCM byte buffer is additional, as are block metadata, queues, atomics, CPAL/backend storage and thread stacks. The worker requests a 1-MiB stack. These fixed ownership bounds are not a total-process RSS claim.

The worker checks generation after reading and before publication. Full-queue waits use short worker-thread parking, never callback spinning. Paused callbacks may retire stale blocks so seeks can refill while the device stream continues emitting silence. Shutdown wakes full-queue/EOF waits before joining. Ring blocks contain no heap-owned fields; callback allocator regressions check both allocations and deallocations.

## CLI

Build with the optional native feature:

```sh
cargo build -p daiw-audio --features native --bin daiw-play
target/debug/daiw-play --help
target/debug/daiw-play --stream-wav "$ABS_WAV" --sha256 "$SHA256" --validate-only
target/debug/daiw-play --stream-wav "$ABS_WAV" --sha256 "$SHA256" --start-frame 48000
target/debug/daiw-play --stream-wav "$ABS_WAV" --sha256 "$SHA256" --loop-start 48000 --loop-end 96000
```

`--validate-only` prints a newline-terminated `WavFileDescriptor` without opening an audio device. Start must be inside the file; loop flags must appear together and describe a valid range. Unknown, missing or duplicate flags fail explicitly. Non-loop playback exits at EOF or a surfaced fault. Loop CLI playback continues until process interruption such as Ctrl-C; the library exposes explicit graceful shutdown, while process interruption is not a claim of a drained device queue.

The CPAL adapter enumerates supported output configurations and selects exact f32 stereo 48-kHz support, including valid non-default configurations. It does not silently resample or change channel layout. The resident adapter uses the shared exact-format selector for its own requested dimensions. Device errors are counted atomically, and ordinary domain pause does not suspend the CPAL stream.

## Verification and remaining limits

Headless tests cover exact file/digest/header bounds, same-handle pathname replacement, signed conversion, variable callback sizes, block crossings, one-frame/unaligned loops, pause/seek/stop, EOF, underrun, runtime read failure, queue-full shutdown, generation races and callback allocation behavior. Forced-interleaving tests target control/callback publication seams rather than relying on repeated timing races.

An independent real-file driver consumed a partial block, paused, sought across frame 4,095, looped over `[5,8)`, and shut down twice. It exposed a stale partial-block offset bug that unit coverage initially missed; the corrected path emits frame 4,095 rather than 4,103, and pause without seek still resumes at frame 8. CLI help, valid validation-only output and wrong-checksum rejection were also exercised.

Physical WASAPI/ALSA/PipeWire devices, audible output, driver latency, xrun rates and Windows execution were not available for validation. Native-feature compilation and pure configuration tests do not certify those behaviors. Desktop native-control integration, live graph/DSP execution, recording, longer-session policy and broad device qualification remain open.
