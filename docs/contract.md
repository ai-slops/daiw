# Stage 1 integration contract

All JSON fields use camelCase. Rust is the sole canonical domain authority. Node stores opaque Rust-produced snapshots transactionally in SQLite and supplies the authenticated principal. The client never sends a full replacement state.

## Domain wire types

`Project`: `{ id, name, revision, tempo, meter:[4,4], definitions:Definition[], instances:Instance[], tracks:Track[], modifiers:Modifier[], proposals:Proposal[], history:HistoryEntry[] }`.

`Note`: `{ id, start, duration, pitch, velocity }`. Beat values on the wire are numbers quantized to 1/960 beat by the boundary; Rust stores/validates exact ticks internally or rejects off-grid values. Pitch may be fractional cents expressed in semitones.

`Definition`: `{ id, name, length, notes:Note[], revision }`.

`Track`: `{ id, name, role, color, instrument, gain, pan, mute, solo, cutoff }`. Instruments: `synth`, `bass`, `kick`, `snare`, `hat`, `pad`, `sample`. Gain is dB, pan -1..1, cutoff Hz.

`Instance`: `{ id, name, definitionId, trackId, start, length, modifiers:string[], overrides:Operation[] }`.

`Modifier`: `{ id, name, operations:Operation[] }`.

`Operation` is tagged by `type`: `transpose` `{semitones}`, `velocity` `{factor}`, `gain` `{db}`, `addNotes` `{notes:Note[]}`, `replaceSlot` `{slot:string, notes:Note[]}`, `setParameter` `{parameter:"cutoff"|"pan", value:number}`. All operate in the target instance context; repeated same-slot replacement or same-parameter setters must conflict, not last-write-win.

`Proposal`: `{id,name,request,baseRevision,instanceId,status,hunks:Hunk[]}`. Status `pending|accepted|partiallyAccepted|rejected`. `Hunk`: `{id,title,description,operation:Operation,dependencies:string[],accepted:boolean}`.

`HistoryEntry`: `{id,message,author,revision}`. Revisions and IDs are strings, never array indices. Synthetic fixture IDs can be descriptive; new entities use random UUIDs supplied by Rust.

`EffectiveNote`: Note plus `{instanceId,trackId,origin:{definitionId,noteId,modifierIds:string[]}}`; start is project beat. `Snapshot`: `{revision,tempo,length,tracks:Track[],notes:EffectiveNote[],regions:AudioRegion[]}`. `AudioRegion`: `{id,trackId,assetId,start,duration,gain,sourceOffset}` (beats except sourceOffset seconds). `Project` may include `regions:AudioRegion[]` and `assets:Asset[]` with defaults. `Asset`: `{id,name,hash,frames,sampleRate,channels}`.

## Worker protocol

One JSON object per line on stdin/stdout. stderr is diagnostics only.
Input: `{id:string,principal:"human"|"agent",state:Project|null,method:string,params:object}`.
Success: `{id,ok:true,state:Project,result:object}`. Failure: `{id,ok:false,error:{code,message}}`.
No state writes outside Rust commands. `state:null` is allowed only for `project.create`.

Methods:
- `project.create` params `{name?,fixture?:boolean}`. Default shared-chorus fixture, blank when fixture false.
- `project.get`, `project.validate`, `project.snapshot` (optional `{proposalId}` to render a proposal), `project.explain` `{instanceId}`.
- `change.dryRun` / `change.apply` params `{expectedRevision,command:Command}`. Dry-run returns `{executable,affectedInstances:string[],hunks:object[],diagnostics:Diagnostic[]}` without modifying state. Apply human-only. Errors must preserve state.
- `proposal.create` params `{expectedRevision,instanceId,request,operations?:Operation[]}`. Limited deterministic natural-language adapter supports octave up/down, softer, louder, brighter, humanize/velocity. Unknown requests return UnsupportedCapability, never claim an LLM. With explicit operations any agent can propose in scope.
- `proposal.accept` params `{expectedRevision,proposalId,hunkIds:string[]}` human-only; enforce dependency closure and stale base, accepted hunks become instance overrides.
- `proposal.reject` params `{expectedRevision,proposalId}` human-only.
- `project.undo` / `project.redo` handled through opaque prior canonical snapshots supplied by storage but must advance revision to reject stale commands. Coordinate with lead.

Commands tagged by `type`:
- `setTempo` `{tempo}`
- `renameProject` `{name}`
- `addTrack` `{name,instrument,color,role}`
- `setTrack` `{trackId,gain?,pan?,mute?,solo?,cutoff?,name?}`
- `addDefinition` `{name,length,notes}`
- `instantiate` `{definitionId,trackId,start,name?}`
- `moveInstance` `{instanceId,start,trackId?}`
- `duplicateInstance` `{instanceId}`
- `deleteInstance` `{instanceId}`
- `editNotes` `{instanceId,scope:"source"|"instance",notes:Note[]}`
- `applyModifier` `{instanceId,modifierId}`
- `removeModifier` `{instanceId,modifierId}`
- `createModifier` `{name,operations}`
- `addAudioRegion` `{asset:Asset,region:AudioRegion}` trusted upload adapter, human only
- `removeAudioRegion` `{regionId}`

`Diagnostic`: `{code,severity:"blocking"|"warning"|"info",message,entityId?:string}`.

## HTTP host (lead owns)

GET `/api/session` -> `{token}` per-process session (same-origin only).
GET `/api/projects` -> `{projects:[{id,name,revision,updatedAt}]}`.
POST `/api/rpc` body `{projectId?:string,method,params,idempotencyKey?:string}`; X-DAIW-Session header required for mutations. Returns worker response. Project creation omits projectId. GET `/api/projects/:id/export` returns canonical JSON download. POST `/api/projects/:id/import-audio` multipart upload handled by host (details coordinated).
GET `/api/projects/:id/render.wav?proposalId=...` returns revision-bound PCM WAV and X-DAIW-Revision. Rust `daiw-render` reads Snapshot JSON from stdin, writes WAV stdout; sample assets supplied by host using additional renderer input `assets:{[assetId]:{sampleRate,channels,samples:number[]}}` if supported. Browser clients can alternatively schedule Snapshot in their audio layer; all use the same effective snapshot.

## Ownership

Core worker: `crates/core/**`, domain/commands/proposals/resolver/CLI tests. Audio worker: `crates/audio/**`, `src/audio/**`, native backend tests and docs/audio.md. UI worker: `src/**` except `src/audio/**`, index.html, DESIGN.md. Lead: server, storage, integration, root manifests, scripts, desktop shell and project docs. No worker commits or changes `.dim`.
