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

1. Query `project.get` or `project.explain`.
2. Send `change.dryRun` with `expectedRevision` and a typed command.
3. Human edits use `change.apply`; assistant edits use `proposal.create`.
4. Fetch `project.snapshot` or `render.wav?proposalId=...` to audition the proposal.
5. Human approval uses `proposal.accept` and a list of hunk IDs. Dependencies and stale bases are checked again.

`project.undo` and `project.redo` accept `expectedRevision`. Their snapshot sources come from the host's durable history; client-supplied `snapshot` fields are rejected.

## Optional agent endpoint

Set `DAIW_AGENT_TOKEN` in the host environment, never in a project. `POST /api/agent` accepts the same envelope with `Authorization: Bearer <token>`.

Agents may query, dry-run, and create instance-scoped proposals. They cannot call `change.apply`, approve/reject, create/import projects, or undo/redo. The host allowlist and Rust authorization both enforce the boundary. A future multi-project deployment should issue short-lived per-project capabilities; this initial token covers the local workspace's readable projects and must not be shared as a public credential.

## Audio and export

- `POST /api/projects/:id/import-audio`: multipart `file`, `trackId`, `start` (beats), and `expectedRevision`. Send `X-DAIW-Idempotency` to safely replay a lost upload response. Original sources are immutable. Limits: 20 MB upload, 180 seconds, and 4 million decoded interleaved samples per source.
- `GET /api/projects/:id/render.wav`: deterministic stereo PCM export; optional `proposalId` selects a proposal snapshot. Optional `expectedRevision` rejects stale audition requests. The browser checks `X-DAIW-Revision` before installing audio buffers.
- `GET /api/projects/:id/assets/:assetId/peaks`: 128 measured waveform bins plus source metadata. Missing/corrupt sources are errors, not fabricated waveforms.
- `GET /api/projects/:id/export`: canonical native JSON. This is not a portable media archive; source blobs remain in the project store.
- `project.import`: explicitly validate an exported project and create a new project identity. Missing source assets are reported when audio is resolved/rendered.

Offline rendering is limited to three-minute arrangements and 8 million source PCM samples in this initial host to bound memory and processing. These are explicit implementation limits, not claims that full-session streaming is complete. Larger sessions and disk streaming remain an outstanding part of the original stage-1 target.

## Error semantics

`StaleRevision` returns HTTP 409. Authorization failures return 403. Validation/domain failures return 400 or 422. Size limits return 413. Worker failures do not move HEAD. Errors should be shown in the client, not converted into apparent success or empty default projects.

## Storage configuration

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
