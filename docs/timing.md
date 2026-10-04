# Tempo and meter maps

Tempo controls beat-to-seconds mapping. Meter controls bar numbering and navigation independently; it does not change elapsed time. The quarter-note beat remains the wire unit and the canonical grid remains 960 ticks per beat.

## Canonical wire contract

Projects may omit either map to retain constant scalar timing:

```json
{
  "tempo": 120,
  "tempoMap": {"points": [
    {"at": 0, "bpm": 120, "curve": "linear"},
    {"at": 8, "bpm": 180, "curve": "step"}
  ]},
  "meter": [3, 4],
  "meterMap": {"points": [
    {"at": 0, "numerator": 3, "denominator": 4},
    {"at": 6, "numerator": 5, "denominator": 4}
  ]}
}
```

Each map has 1–1024 strictly increasing tick-aligned points, starts at zero, and stays within 1,000,000 beats. Tempo is finite and 20–400 BPM. A point's curve describes the segment leaving that point; the last curve must be `step`. Meter components are integers 1–255, preserving the previous positive-u8 contract rather than introducing a power-of-two restriction. Meter changes must occur on an exact bar boundary of the preceding meter. The scalar tempo/meter must equal the first corresponding map point.

Commands under normal human/expected-revision authorization:

- `setTempoMap {tempoMap}` replaces the tempo map and synchronizes scalar tempo.
- `setMeterMap {meterMap}` replaces the meter map and synchronizes scalar meter.
- `setTempo {tempo}` explicitly returns to constant tempo by clearing its map.
- `setMeter {meter}` explicitly returns to constant meter by clearing its map.

Agents can dry-run, not apply. Invalid maps do not commit. Timing changes participate in impact reporting. Snapshot data carries tempo-map timing to rendering and musical meter to the client. Native exports declare `tempo-map-v1` and/or `meter-map-v1`; absent optional fields preserve the old native content hash. Unknown-feature clients must reject mapped projects rather than flatten them silently.

## Conversion and audio behavior

`daiw-timing` is a leaf crate used by core and audio; audio does not depend on core. It owns validated maps and Tick, cached prefix integrals and inverse conversion. Linear BPM ramps integrate using `ln_1p` and invert using `exp_m1`; equal endpoints use the constant formula. Public Rust conversions reject invalid/out-of-domain inputs, and inverse results quantize to the nearest tick. Node/browser use one shared ESM implementation, matching tick tolerance, limits and integer meter-boundary validation. Browser inverse positions remain continuous for playhead display.

Mapped render durations use rounded absolute end-frame minus start-frame. Scalar rendering retains its existing duration rounding and waveform behavior. Regions remain **unwarped**: tempo affects placement and gating, while source samples advance in wall-clock time from a source offset measured in seconds. Imported source duration is rounded upward to a tick in both modes so its tail is not truncated. Silence after source end remains silence.

The host's 180-second render limit uses the integrated tempo map. Browser seek offsets, elapsed position and loop endpoints use mapped seconds. A seek at or beyond an active loop end is normalized before playback scheduling, keeping audible and displayed positions consistent.

## Editor

Open **Tempo & meter maps** from the transport BPM control. Edit point tables, add/remove rows, and save each map independently. Constant controls explicitly clear the corresponding map. Cancel/reopen discards unsaved drafts, but does not undo a map already saved. Saving disables competing timing controls; success is shown only after the canonical revision changes, and errors remain visible inside the dialog.

Transport shows the active meter and bar/beat/tick position. Navigation and the four-bar loop follow meter changes; the arrangement ruler uses bounded meter-aware marks. This is a point editor, not graphical tempo automation. Warp/stretch, recording/count-in, external clock synchronization, tempo proposals, nested composition and full-session streaming remain separate work.
