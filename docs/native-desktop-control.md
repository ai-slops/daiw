# Desktop native playback control

The desktop editor can explicitly select Native output for the current arrangement's verified rendered WAV. Browser remains the default and retains A/B audition, loudness matching and metering. Native output does not silently fall back to Browser when preparation or device opening fails.

This connects the existing disk-streamed WAV player to editor controls. It is not live synthesis, live execution of the compiled signal graph, recording, synchronized native A/B, or physical-device certification.

## Setup and usage

```sh
npm run core:build
npm run native:build
npm run build
npm run desktop
```

The native build enables the optional CPAL feature. Linux requires ALSA development headers; the desktop shell also needs its normal shared libraries and a display server. Ordinary `core:build` alone does not build `daiw-play`. Set `DAIW_PLAY_BINARY` to an absolute path to use an alternate native player binary. Missing binaries report Native unavailable without exposing host paths.

In the transport, choose **Output: Native**, select a supported output device, then use the existing play/pause/stop, bar seek and four-bar loop controls. Retry device enumeration after connecting a device. Devices must support exact f32 stereo 48 kHz output; no callback resampling or format fallback is added. A plain browser cannot access the native bridge.

Native A/B and loudness matching controls are disabled with an explanation; direct calls fail with a typed unsupported-capability error. The mixer says Native metering is unavailable instead of presenting silent placeholder values as measurements. Switching output waits for the previous backend to release before enabling the next one.

## Authority and process boundary

```text
React transport -> sandboxed preload -> Electron main
              -> Node server child IPC -> native controller
              -> fixed daiw-play executable -> CPAL / WAV worker
```

The preload exposes individual named methods, not arbitrary IPC, filesystem or process-spawn access. Electron requires the application's main frame at the exact local origin and supplies its own client identity. The native endpoint exists only on the Node child IPC channel, not HTTP or the agent RPC surface.

Preparation accepts project ID, expected revision and an opaque device token. Only the host renders and chooses the canonical managed WAV path, expected SHA-256, binary and device selector. `managedPath` is an internal verified render result field and is not added to HTTP render metadata. Rust opens and verifies the WAV independently before reporting ready; Node then closes its render handle.

Device tokens belong to a client and remain valid until enumeration refresh, client release or controller shutdown. Session release preserves the device list so repeated preparation and backend switching work. Production preparation requires a supported host-issued token before expensive rendering. Rust re-enumerates devices and verifies the selected index and name digest; duplicate names are unsupported because these fields cannot distinguish same-named physical devices safely. Tokens are not persistent device IDs.

## Controlled worker protocol

The previous snapshot, stdin snapshot and one-shot `--stream-wav` modes remain available. New modes are:

```text
daiw-play --list-output-devices --json
daiw-play --control-wav ABS --sha256 HEX --device-index N --device-sha256 HEX
```

Enumeration returns version 1 and device index/name/name-SHA/default/support metadata. The controlled process emits a `ready` JSON line followed by correlated responses and monotonically sequenced state events. Its strict versioned commands are `play`, `pause`, `stop`, `seek`, `setLoop`, `state` and `shutdown`:

```json
{"v":1,"id":1,"type":"seek","frame":24000}
{"v":1,"id":2,"type":"setLoop","loop":{"start":48000,"end":96000}}
{"v":1,"id":3,"type":"shutdown"}
```

Positions and end-exclusive loop bounds are stereo frames, not interleaved sample offsets. The UI converts using the snapshot's tempo map. State distinguishes initial/explicit stopped, paused, playing and faulted, with frame position, loop and stream counters. EOF reports stopped; seeking after EOF does not falsely claim playback until explicitly started.

Input/output lines are capped at 4,096 bytes and the Rust input queue has eight slots. The host bounds pending commands and subprocess output, handles broken pipes without crashing, and terminates an unresponsive worker. Commands emit immediate state; active playback also emits periodic state updates. Startup failures use fixed safe codes/messages, distinguishing invalid WAV from device/backend failures.

## Lifecycle and failures

- Each client owns at most one prepared session. New ownership is installed before awaiting previous-worker cleanup, so the latest concurrent preparation wins.
- Preparation is cancelable through rendering and worker readiness. Superseded children and handles are closed, including children that have not yet reported ready. Revision is checked before preparation, after ready and before play.
- Renderer preparation is single-flight. State, errors and busy completion are accepted only for the current generation/session/project/revision; stale async failures cannot overwrite a newer successful state.
- Unexpected worker exit clears the matching host session, emits a visible tagged fault and permits explicit retry. Intentional shutdown and superseded generations do not create spurious faults.
- Electron preparation timeouts cancel the matching request, not a newer session. Navigation/reload, renderer process loss, window destruction and application quit release owned sessions.
- Shutdown joins the WAV disk worker even when stdin remains open. The dedicated process does not wait indefinitely for its blocking input reader; process termination retires that reader. This is not an in-process reusable reader API.

## Verification and limitations

Final validation passed **344 all-feature Rust tests**, **122 host tests** and **106 client tests**, all-feature build/Clippy, formatting and TypeScript/Vite build. Regressions cover open-stdin shutdown/fatal exit, EOF state, device ambiguity, safe startup failures, stale revisions, superseded startup, slow-close overlapping preparation, token lifecycle, worker death/retry, pipe errors, command timeout and IPC sender/cleanup boundaries.

An independent integration driver used actual `ProjectService`, `MediaStore`, the native controller and Rust WAV worker/callback to render a fixture, play/advance 256 frames, pause, seek to frame 4,095, set loop `[5,8)`, stop at zero and release. It then drove the actual React UI in Chromium through two preparations and Native/Browser transitions with zero console warnings/errors. The desktop bridge was explicitly a QA adapter backed by the real headless controller, not an actual Electron IPC instance or audio device.

Fresh screenshots and two independent visual reviews covered Browser-only availability, Native playing, and no-device/retry states at 1,280, 768 and 375 pixels. Deferred-response browser scenarios also verified same-tick single-flight preparation, worker-fault retry and rejection of stale preparation/command/release errors.

Physical enumeration in this container returned no devices. Headed Electron could start its service but could not render without an X display. Actual Windows/Linux device opening, audible output, latency/xruns, full Electron-window operation, hotplug qualification and background/long-session behavior remain open. Headless callback tests and a browser bridge fixture do not certify those behaviors.
