# Local API

The HTTP host defaults to port 3000. The desktop shell uses a dedicated loopback port. DIM can expose the HTTP client using its development-service gateway. There is no unauthenticated remote-control mode with privileged mutations.

## Session and projects

```sh
curl http://localhost:3000/api/session
curl http://localhost:3000/api/projects
```

`GET /api/session` returns a session token. Send it in `X-DAIW-Session` for all GUI POST requests. The token rotates on service restart. Do not store it in projects or logs.

```http
POST /api/rpc
Content-Type: application/json
X-DAIW-Session: <token>

{"method":"project.create","params":{"name":"My song","fixture":true},"idempotencyKey":"create-song-1"}
```

Responses use `{id,ok:true,state,result}` or `{ok:false,error:{code,message}}`. The returned `state.revision` is the base for the next mutation. See [contract.md](contract.md) for command shapes.

## Editing and review

Definition-owned child placements are available through `addDefinitionChild`, `setDefinitionChild`, `removeDefinitionChild` and `editDefinitionNotes`. `project.explain.compositionNotes` exposes pre-modifier nested provenance; source edits remain own-note-only. See [composition.md](composition.md) for wire shapes, bounds, dependency guards and UI behavior.

Tempo/meter maps and `setTempoMap`, `setMeterMap`, `setMeter` are now supported alongside `setTempo`. Constant setters clear the corresponding map. See [timing.md](timing.md) for wire examples, validation, mapped render/seek semantics and the point-table editor.

1. Query `project.get` or `project.explain`.
2. Send `change.dryRun` with `expectedRevision` and a typed command. Inspect both structural `executable` and optional scoped `noteResolution.ready`, not just the former.
3. Human edits use `change.apply`; assistant edits use `proposal.create`.
4. Fetch `project.snapshot` or `render.wav?proposalId=...` to audition the proposal.
5. Human approval uses `proposal.accept` and a list of hunk IDs. Dependencies and stale bases are checked again.

`project.undo` and `project.redo` accept `expectedRevision`. Their snapshot sources come from the host's durable history; client-supplied `snapshot` fields are rejected.

Dry-run now reports affected instance dependencies and before/after beat ranges in `impact`, plus per-instance `absent`, `clean`, or `conflict` checks in `noteResolution`. A source edit that deletes a locally overridden note reports its stable ID and suggested resolution actions before rendering, without advancing HEAD. Old clients still receive the candidate conflicts in `diagnostics`; new clients preserve structured reports. See [the full scoped contract](contract.md#scoped-dry-run-impact-and-note-resolution-reports).

Example selected result fields for a project with one affected instance:

```json
{
  "executable": true,
  "noteResolution": {
    "ready": false,
    "instances": [{
      "instanceId": "instance-a",
      "before": {"status":"clean","definitionId":"definition-a","definitionRevision":1,"noteCount":4},
      "after": {
        "status":"conflict","definitionId":"definition-a","definitionRevision":2,
        "diagnostic":{"code":"RebaseConflict","severity":"blocking","message":"note patch cannot be rebased: missing note target note-a","entityId":"instance-a"},
        "target":{"kind":"missing","noteId":"note-a"},
        "actions":["restoreMissingNote","editInstanceNotes"]
      }
    }]
  }
}
```

`executable:true` means the canonical edit may be saved, not that playback is ready. These checks cover affected first-loop note patterns only. Still request a full snapshot/render for arrangement-wide or audio failures. Reports do not imply automatic rebase or approval.

## Optional agent endpoint

Set `DAIW_AGENT_TOKEN` in the host environment, never in a project. `POST /api/agent` accepts the same envelope with `Authorization: Bearer <token>`.

Agents may query, dry-run, and create instance-scoped proposals. They cannot call `change.apply`, approve/reject, create/import projects, or undo/redo. The host allowlist and Rust authorization both enforce the boundary. A future multi-project deployment should issue short-lived per-project capabilities; this initial token covers the local workspace's readable projects and must not be shared as a public credential.

## Audio and export

- `POST /api/projects/:id/import-audio`: multipart `file`, `trackId`, `start` (beats), and `expectedRevision`. Send `X-DAIW-Idempotency` to safely replay a lost upload response. Original sources are immutable. Limits: 20 MiB upload, 180 seconds, and 512 MiB decoded PCM payload per source. The old 4-million-sample host cap is replaced by file-backed decoding.
- `GET /api/projects/:id/render.wav`: deterministic stereo PCM export; optional `proposalId` selects a proposal snapshot. Optional `expectedRevision` rejects stale audition requests. The browser checks `X-DAIW-Revision` before installing audio buffers.
- `GET /api/projects/:id/assets/:assetId/peaks`: 128 measured waveform bins plus source metadata. Missing/corrupt sources are errors, not fabricated waveforms.
- `GET /api/projects/:id/export`: versioned native JSON envelope with `format`, `formatVersion`, `requiredFeatures`, `contentHash`, and `content`. This is not a portable media archive; source blobs remain in the project store.
- `project.import`, with `params.project` containing that envelope or legacy raw project JSON: validate before creating a fresh identity/revision. The result reports `sourceFormatVersion` (1 or legacy 0). Unknown versions/features/semantic fields and corrupt content are rejected without replacing HEAD. Missing or mismatched source assets fail when audio is resolved/rendered.

The Projects dialog provides **Import project** for native or legacy JSON files. Successful import opens the new project; failed import leaves the current project open and displays an error. The client does not strip unknown content before the core compatibility check.

## Bounded note selection

`project.queryNotes` accepts `{instanceId,scope,selector,maxResults}` and returns `{selection}`. Scope is `source` or `instance`; `maxResults` must be 1–4096. Selectors are `all`, `stableIds {noteIds}`, `namedSlot {slot}`, `beatRange {start,end}` and `pitchRange {min,max}`. Beat ranges use half-open note-onset membership, pitch bounds are inclusive, and `notes` is the virtual whole-pattern slot. Missing slots or requested IDs are errors; exceeding the result limit returns 413, not truncation.

Pass the returned selection unchanged to `transformSelectedNotes {selection,transform}` through normal dry-run/apply methods. Transforms are `transpose {semitones}` and `setVelocity {velocity}`. The handle binds project and definition revisions, scope and resolved IDs; stale handles return 409 `SelectionStale`. Empty or unchanged transforms return `InvalidParams` without advancing source revision. Agents may query and preview but cannot apply. Instance edits retain existing whole-note override semantics, not field-level source merging. See [native exchange and selection](native-exchange.md) for integrity and scope limits.

Offline rendering remains limited to three-minute arrangements. Source PCM uses bounded file reads, with at most 64 distinct files and 1 GiB of referenced decoded file bytes per render; the managed decoded cache budget is 4 GiB. Output is now rendered to verified WAV files and transferred with HTTP backpressure, under a separate 2 GiB render-cache budget. Headers are sent only after complete rendering and verification; no range API is added. Browser audition still buffers the complete WAV. See [source streaming](offline-streaming.md) and [output streaming/cancellation](output-streaming.md).

The Rust renderer additionally caps output at 8,640,000 stereo frames (180 seconds at the host's 48 kHz), checks frame conversions before allocation and rejects invalid frequency/phase/gain/mix results. Non-finite audio is never converted into a successful silent/clipped WAV. Finite out-of-unity samples retain normal PCM clipping. A finite-note snapshot can succeed while its render returns **422 `AudioError`**; only a real render establishes DSP readiness. Failed renders leave HEAD and render-cache rows/files unchanged. Deployments must restart the host after replacing the renderer binary to refresh the cache fingerprint. See [audio.md](audio.md#numeric-and-output-memory-boundaries).

## Error semantics

`StaleRevision` returns HTTP 409. Authorization failures return 403. Validation/domain failures return 400 or 422. Size limits return 413. Worker failures do not move HEAD. Errors should be shown in the client, not converted into apparent success or empty default projects.

## Storage configuration

`project.cacheStats` and `project.instanceCacheStats` are authenticated, human-only read-only queries for the worker's source-composition and post-modifier instance-pattern caches respectively. Neither alters HEAD, the command journal or the other cache's diagnostics, and neither is available through the agent allowlist. Read them immediately after the request being measured. See [source-cache diagnostics](incremental-resolution.md) and [instance-cache diagnostics](instance-resolution.md) for their shared shape and worker-wide scope.

| Variable | Default | Meaning |
|---|---|---|
| `PORT` | `3000` | HTTP port |
| `HOST` | `127.0.0.1` | Bind address; use `0.0.0.0` only when deliberately exposing the service |
| `DAIW_DATA_DIR` | `data` | SQLite, immutable source blobs, and caches |
| `DAIW_CORE_BINARY` | `target/debug/daiw-core` | Rust command worker |
| `DAIW_RENDER_BINARY` | `target/debug/daiw-render` | Rust renderer/decoder |
| `DAIW_AGENT_TOKEN` | unset | Enables separately restricted agent requests |
| `DAIW_ALLOWED_ORIGINS` | unset | Comma-separated explicit public origins for a trusted reverse proxy |

The host rejects unrecognized Host headers, including same-origin DNS-rebinding attempts. Only loopback proxy connections may supply forwarded hosts. In DIM, the URL returned by `dim-development-service expose` is registered automatically as an allowed origin; arbitrary forwarded hostnames are not trusted.
