# Stage 1 integration contract

All JSON fields use camelCase. Rust is the sole canonical domain authority. Node stores opaque Rust-produced snapshots transactionally in SQLite and supplies the authenticated principal. The client never sends a full replacement state.

The long-lived worker owns a bounded source-composition cache. Cache contents are derived and process-local; full state remains supplied per request. Human-only `project.cacheStats` exposes noncanonical diagnostics. [Incremental-resolution semantics](incremental-resolution.md) define invalidation, logical budget replay, retention accounting and the stateless-library fallback.

Native interchange uses a separate versioned envelope, not worker `state`. See [native exchange and bounded selection contracts](native-exchange.md) for required features, typed-content hashing, compatibility rejection, `SelectionHandle`, and `transformSelectedNotes` semantics.

## Domain wire types

Optional `Definition.children`, nested note provenance and descendant revision closures are specified in [composition.md](composition.md). These require `nested-definitions-v1` in native documents. Empty children are omitted for flat-project compatibility.

Optional `Project.tempoMap` and `Project.meterMap`, their setters, native feature requirements and snapshot behavior are specified in [the timing contract](timing.md). Omitted maps preserve constant timing; they are not silently inferred from scalar fields.

`Project`: `{ id, name, revision, tempo, meter:[4,4], definitions:Definition[], instances:Instance[], tracks:Track[], modifiers:Modifier[], proposals:Proposal[], history:HistoryEntry[] }`.

`Note`: `{ id, start, duration, pitch, velocity }`. Velocity is normalized `0..1`, not MIDI `0..127`. Beat values on the wire are numbers quantized to 1/960 beat by the boundary; Rust stores/validates exact ticks internally or rejects off-grid values. Pitch may be fractional cents expressed in semitones.

`Definition`: `{ id, name, length, notes:Note[], revision, slots?:{[name]:{start,end}} }`. Definition revision is a nonnegative integer; project commit revisions are strings.

`Track`: `{ id, name, role, color, instrument, gain, pan, mute, solo, cutoff }`. Instruments: `synth`, `bass`, `kick`, `snare`, `hat`, `pad`, `sample`. Gain is dB, pan -1..1, cutoff Hz.

`Instance`: `{ id, name, definitionId, trackId, start, length, modifiers:string[], overrides:Operation[], modifierBindings?:{[modifierId]:ModifierBinding}, modifierOrdering?:{edges:{before:string,after:string}[]} }`.

`Modifier`: `{ id, name, operations:Operation[] }`.

`Operation` is tagged by `type`: `transpose` `{semitones}`, `velocity` `{factor}`, `gain` `{db}`, `addNotes` `{notes:Note[]}`, `replaceSlot` `{slot:string, notes:Note[]}`, `setParameter` `{parameter:"cutoff"|"pan", value:number}`. All operate in the target instance context; repeated same-slot replacement or same-parameter setters must conflict, not last-write-win.

Instance overrides additionally serialize the internal operation `patchNotes` `{updates:Note[], added:Note[], removed:string[]}`. `editNotes` with instance scope creates it from the complete editable first-loop pattern and normalizes previous note patches into one final difference against current source/modifiers/non-patch overrides. It is rejected in reusable modifiers and proposal hunks, including imported canonical projects. Clients may read it but must use `editNotes`, not submit it as a reusable operation.

Patch updates retain stable note IDs and replace the entire note record. Unchanged notes are not copied, so future unrelated source updates/additions/deletions propagate. Cancelling local changes removes their patch dependency. Note submissions must fit `min(definition.length, instance.length)`; persisted patch updates/additions must fit the definition. Missing targets, ambiguous effective IDs, and added-ID collisions yield `RebaseConflict` at snapshot/render and are reported in dry-run note-resolution checks. Saving a source change can still leave an unresolved canonical state; dry-run is not a full compile gate. The targeted `project.explain` path does not require unrelated instances to render successfully.

Legacy `replaceSlot` overrides keep their original full-range replacement meaning; they are not silently converted to inferred note edits. Projects containing `patchNotes` require a core/client that understands that operation. Older readers fail on the unknown enum tag rather than safely supporting it; full feature negotiation/migration remains outstanding.

`Proposal`: `{id,name,request,baseRevision,instanceId,status,hunks:Hunk[]}`. Status `pending|accepted|partiallyAccepted|rejected`. `Hunk`: `{id,title,description,operation:Operation,dependencies:string[],accepted:boolean}`.

`HistoryEntry`: `{id,message,author,revision}`. Revisions and IDs are strings, never array indices. Synthetic fixture IDs can be descriptive; new entities use random UUIDs supplied by Rust.

`EffectiveNote`: Note plus `{instanceId,trackId,origin:{definitionId,noteId,modifierIds:string[]}}`; start is project beat. `Snapshot`: `{revision,tempo,length,tracks:Track[],notes:EffectiveNote[],regions:AudioRegion[]}`. `AudioRegion`: `{id,trackId,assetId,start,duration,gain,sourceOffset}` (beats except sourceOffset seconds, gain in dB). `Project` may include `regions:AudioRegion[]` and `assets:Asset[]` with defaults. `Asset`: `{id,name,hash,frames,sampleRate,channels}`.

## Worker protocol

One JSON object per line on stdin/stdout. stderr is diagnostics only.
Input: `{id:string,principal:"human"|"agent",state:Project|null,method:string,params:object}`.
Success: `{id,ok:true,state:Project,result:object}`. Failure: `{id,ok:false,error:{code,message}}`.
No state writes outside Rust commands. `state:null` is allowed only for `project.create`.

Methods:
- `project.create` params `{name?,fixture?:boolean}`. Default shared-chorus fixture, blank when fixture false.
- `project.get`, `project.validate`, `project.snapshot` (optional `{proposalId}` to render a proposal), `project.explain` `{instanceId}`.
- `change.dryRun` / `change.apply` params `{expectedRevision,command:Command}`. Dry-run returns `{executable,affectedInstances:string[],hunks:object[],diagnostics:Diagnostic[],impact?,noteResolution?}` without modifying state. Apply human-only. Errors must preserve state. See the scoped report contract below.
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
- `applyModifier` `{instanceId,modifierId,ordering?:{edges:{before,after}[]}}` may atomically attach a new modifier and install the complete explicit ordering graph
- `setModifierOrdering` `{instanceId,ordering:{edges:{before,after}[]}}` replaces the instance's explicit dependency graph
- `rebaseModifier` `{instanceId,modifierId}` refreshes the attached slot modifier's binding; human-only through `change.apply`, revision-bound and dry-runnable
- `removeModifier` `{instanceId,modifierId}`
- `createModifier` `{name,operations}`
- `addAudioRegion` `{asset:Asset,region:AudioRegion}` trusted upload adapter, human only
- `removeAudioRegion` `{regionId}`

`Diagnostic`: `{code,severity:"blocking"|"warning"|"info",message,entityId?:string}`.

## Scoped dry-run impact and note-resolution reports

`executable` retains its existing meaning: the command passes expected-revision, mutation, and canonical structural validation. It does **not** certify audible/renderable output. A structurally valid edit can return `executable:true` and `noteResolution.ready:false`, with blocking **note-resolution** diagnostics. Apply semantics are unchanged so a user can save an unresolved canonical edit deliberately.

After structural validation succeeds, the current core emits both reports. Client schemas make them optional to accept older core responses. Invalid/stale commands return `executable:false`, diagnostics, and **no** speculative impact/resolution report. Dry-run domain failures remain inside the successful query envelope (HTTP 200), unlike failing mutation requests.

`impact`:

```text
{
  baseRevision: string,
  affectedDefinitions: string[],
  affectedTracks: string[],
  previewRanges: [{instanceId, before: {start,end}|null, after: {start,end}|null}]
}
```

Scope is **instance-centric**, not a generic entity diff. Affected instances include explicit command targets plus changed/new/deleted instances and dependents of changed definitions/tracks, tempo, and global solo state. IDs and ranges are ordered deterministically. Definition/track lists are dependencies of those instances, not necessarily directly edited objects. Audio-only edits and uninstantiated entities do not contribute to these lists. Beat ranges cover the full instance before and after an edit; they do not include audio regions, pre-roll, effect tails, or a signal-path render plan. `null` marks absence before creation or after deletion. Generated IDs from instantiate/duplicate dry-runs are **candidate-only**; a subsequent apply generates its own IDs.

`noteResolution`: `{ready:boolean, instances:[{instanceId,before:InstanceCheck,after:InstanceCheck}]}`. Both states use the same `editable_notes` resolver as instance explanation/editing, without requiring unrelated instances to resolve.

`InstanceCheck` is tagged by `status`:

- `absent`: instance does not exist in that state.
- `clean`: `{definitionId,definitionRevision,noteCount,unboundModifierIds?}`. Count is the visible first-loop note count, not total repeated events or selector cardinality.
- `conflict`: `{definitionId,definitionRevision:number|null,diagnostic,target?,binding?,actions,unboundModifierIds?}`. Missing definition revision is `null`. The first resolver failure for that instance is reported; this is not an exhaustive list of every conflicting note or modifier.

For local patch identity failures, `target` is `{kind:"missing"|"ambiguous"|"addedIdCollision",noteId}`. The stable outer error code remains `RebaseConflict`. Other resolver errors retain their original codes without a fabricated note ID. `actions` contains advisory identifiers: `restoreMissingNote`, `editInstanceNotes`, `chooseUniqueNoteIds`, `reviewModifiers`, or `reviseCommand`. They do not execute automatically or grant agent mutation permission.

Slot-modifier drift instead sets `binding:{modifierId,reason,slot?}` and recommends `actions:["rebaseModifier"]`. Reasons are `definitionChanged`, `sourceRevisionChanged`, `selectorsChanged`, `slotMissing`, `rangeChanged`, `cardinalityChanged`, and `targetsChanged`. Both clean and conflicting checks include nonempty `unboundModifierIds` when legacy slot-bearing attachments are present. A clean result means current resolution succeeded, not that these legacy attachments were verified.

`ready` means no affected candidate instance has a first-loop note-resolution failure. Candidate conflicts are also copied to top-level `diagnostics` with the instance ID, allowing existing diagnostic consumers to display them. `before` and `after` distinguish newly introduced, pre-existing, changed, and resolved failures; corrected/deleted failures do not remain in candidate diagnostics. With no affected instances, `ready:true` is a vacuous scoped result, not a whole-project health check.

This does **not** replace full `project.snapshot`/render checks for whole-arrangement expansion budgets, signal tracks/DSP, missing audio, output limits, or hardware. The slot binding/rebase subset below is supported; arbitrary selectors, full application entities, full impact/cache invalidation and a dedicated rebase UI remain future work.

## Persisted strong slot-modifier bindings

On first `applyModifier`, every `replaceSlot` in that reusable modifier is resolved against the instance's canonical source definition. Its attachment records:

```json
{
  "modifierBindings": {
    "modifier-fill": {
      "definitionId": "def-chorus",
      "definitionRevision": 0,
      "slots": {
        "ending-fill": {
          "range": {"start": 6, "end": 8},
          "noteIds": ["note-chorus-four"],
          "cardinality": 1
        }
      }
    }
  }
}
```

This is a **Strong-only, canonical slot-selector subset**. Named slots select source notes by onset in `[start,end)`, preserving canonical note order. Virtual `"notes"` selects the entire source pattern with range `[0,definition.length)`. IDs refer to source notes, not repeated effective events or notes generated by preceding modifiers. Replacement operations still act on the effective pattern in existing modifier order; the record does not pin every effective contributor in that range. No arbitrary selector AST, Adaptive/Snapshot mode, operation hash or multiple applications of one modifier to the same instance is added.

Bound applications require the same source ID, exact source revision, exact `replaceSlot` selector inventory, ranges, IDs and cardinalities. Any mismatch blocks snapshot/instance explanation with `RebaseConflict` and appears in dry-run before/after checks. **Even an unrelated source edit that advances the definition revision requires explicit rebase.** This conservative policy avoids guessing that a replacement is still intended. A mixed modifier containing both a slot replacement and other operations is guarded as one attachment. Non-slot modifiers keep their existing behavior and are not labeled unbound.

`rebaseModifier` refreshes only the binding from the current source. It does not edit notes, modifier operations, local overrides, or other attachments. It requires an attached slot-bearing modifier; unattached/unknown targets fail with `NotFound`, non-slot modifiers with `UnsupportedCapability`, missing slots or out-of-range replacement notes with `InvalidProject`. Fresh binding creation/rebase validates replacement fit even for virtual `"notes"`. The usual expected project revision and human mutation permission apply. An agent may dry-run but cannot apply the rebase. Unrelated resolver failures may remain after successful metadata refresh; inspect the dry-run candidate and render again.

Repeated `applyModifier` on an existing attachment is **not** a rebase and never refreshes stored metadata, including legacy missing metadata. `removeModifier` removes the matching binding. Instance duplication copies its attachment bindings. Storage, undo/redo snapshots and native JSON exports include the metadata automatically.

Import validates metadata shape, attached map keys, nonempty slot maps, bounded ordered ranges, unique target IDs and exact stored cardinality. Nested metadata fields are required. Historical anchors are allowed to be stale; freshness is checked by resolution, not by silently rewriting imports.

**Legacy compatibility:** a missing/empty outer map preserves prior playback and serialized shape; it does not create verified bindings. `project.explain` returns `unboundModifierIds` for slot-bearing legacy attachments, and dry-run reports them in each check. Explicit `rebaseModifier` upgrades one legacy attachment. Legacy local `replaceSlot` overrides and proposal-hunk bindings retain their existing semantics; this feature guards reusable modifier attachments, not those separate paths.

**Downgrade is unsupported:** older cores may ignore/drop `modifierBindings` and thereby remove these safety checks. Do not open and save newly bound projects with an older core/client. Full native schema/required-feature negotiation is still pending; backward reading by the new build is not a promise of safe forward reading by old builds.

## Explicit modifier dependency ordering

Ordering is opt-in per instance. An omitted `modifierOrdering` preserves exact legacy attachment-array execution. `{"edges":[]}` is deliberately different: explicit mode with no declared dependencies, valid only when its operations do not require ordering. The nested `edges` field is required. New instances currently remain legacy until explicitly opted in; there is no automatic migration to sorted execution.

Example revision-bound command (usable with `change.dryRun` before human `change.apply`):

```json
{
  "expectedRevision": "rev-current",
  "command": {
    "type": "setModifierOrdering",
    "instanceId": "instance-chorus-a",
    "ordering": {
      "edges": [{"before":"modifier-add","after":"modifier-transpose"}]
    }
  }
}
```

Nodes are the instance's attached modifier IDs. An edge means its `before` modifier executes before its `after` modifier. Directed paths count transitively; an arbitrary topological tie-break or shared predecessor/successor does not establish a dependency. Kahn ordering uses stable modifier-ID order for simultaneously ready nodes. The attachment array itself is not rewritten. Operation order inside each modifier is unchanged; instance-local overrides still execute last and cannot be interleaved through this graph.

One planner is used by canonical validation and both note/track resolution. Explicit mode rejects duplicate attachment IDs, unknown endpoints, self-edges, duplicate edges, cycles and undeclared hazardous pairs. Limits are **256 attached modifiers and 4096 raw edges per explicit instance**, checked before graph expansion. These graph limits do not bound every operation payload or replace existing render budgets. Legacy mode retains its existing validation and execution behavior.

### Core-derived read/write declarations

`project.explain` additionally returns `modifierPlan:{mode:"legacy"|"explicit",order:string[],edges?:{before,after}[],modifiers:[{modifierId,reads:Resource[],writes:Resource[]}]}`. Resources are sorted, typed and derived from actual operations, not trusted client declarations. Existing `modifiers` remains in attachment-array order. `modifierPlan.order` is execution order; edges are canonically sorted in commands and reports. Older responses may omit the plan.

| Operation | Reads | Writes |
|---|---|---|
| transpose | `noteMembership`, `notePitch` | `notePitch` |
| velocity | `noteMembership`, `noteVelocity` | `noteVelocity` |
| gain | `trackGain` | `trackGain` |
| addNotes | none | `noteMembership`, `noteTiming`, `notePitch`, `noteVelocity` |
| replaceSlot | `noteMembership`, `noteTiming`, named `slot` | `noteMembership`, `noteTiming`, `notePitch`, `noteVelocity` |
| setParameter | none | keyed `parameter` |

Resource JSON is `{kind:"noteMembership"|"noteTiming"|"notePitch"|"noteVelocity"|"trackGain"}`, `{kind:"slot",slot:string}`, or `{kind:"parameter",parameter:"cutoff"|"pan"}`. These are conservative whole-pattern/instance-strip resources, not exact event-level overlap proofs. Different named slots can therefore require an edge even if their physical ranges do not overlap.

### Conflict rules

- Shared reads alone are independent. A write overlapping another operation's reads/writes requires a declared path unless a compatibility exception below applies.
- Gain deltas are domain-compatible additive contributions. Disjoint `addNotes` IDs are also compatible without edges.
- Transpose/transpose and velocity/velocity require order. In particular, velocity clamps after every operation, so multiplication alone does not establish commutativity.
- Every operation pair across two modifiers is checked. A gain/gain-compatible pair cannot excuse a note hazard elsewhere in mixed modifiers.
- Same-slot replacements and same-key parameter setters remain hard `ModifierConflict` errors even if ordered, in both modes.
- In explicit mode, repeated IDs across **all `addNotes` operations in the resolved stream**, including within one modifier and fixed-last overrides, are hard `ModifierConflict` errors. This prepass runs before undeclared-path reporting. It does not silently weaken existing legacy data acceptance.

`ModifierOrderingInvalid` reports malformed/over-limit graphs; `ModifierOrderingCycle` reports cycles; `ModifierOrderRequired` names a hazardous pair without a declared path. Failed apply/import does not replace state. Dry-run returns `executable:false` with the same code and no speculative resolution report. Agent mutation remains forbidden and stale project revisions still fail normally.

### Attachment lifecycle and limitations

An omitted `applyModifier.ordering` preserves the existing mode. A supplied graph is the **complete** candidate graph after attaching a new modifier, allowing dependencies to be installed atomically. Supplying ordering for an already-attached modifier fails with `InvalidParams`; use `setModifierOrdering` instead. This does not turn repeated apply into a binding refresh.

Removal prunes incident edges, retains explicit mode even when empty, and validates the result. If a removed node was the only path ordering surviving hazardous modifiers, removal is rejected; first declare a surviving bypass edge with `setModifierOrdering`. No automatic bridging or last-write-wins policy is introduced. Duplication and undo/redo snapshots preserve graph presence and edges. Rebase changes binding metadata only, not ordering. Invalid graph structure is a save/import error; stale slot bindings remain separately saveable and block resolution as before.

Gain compatibility is **not a bitwise floating-point commutativity guarantee**. Explicit canonical order determines the exact sum and existing final gain-range validation still applies. Switching from legacy to explicit tie-breaking can change rounding or provenance; retain an explicit chain matching legacy order when exact order preservation is desired. `origin.modifierIds` is an execution trace, not an unordered set.

This is the current operation-algebra/instance-DAG subset, not a general signal graph or arbitrary selector compiler. Final effective-note validation is a separate layer described below. Old cores may ignore/drop `modifierOrdering`, changing execution; **downgrade of explicitly ordered projects is unsupported** until version/required-feature exchange is implemented.

## Final effective-note validation

After modifiers and overrides execute and repetition/cropping produce actual output events, the core validates each emitted note collection. It rejects duplicate effective IDs, non-finite pitch, velocity outside finite `0..1`, negative start, non-positive duration, or an end beyond the timeline policy. End-time arithmetic is checked for integer overflow. The same validation runs over the complete snapshot after aggregating all instances, so collisions between different instances cannot escape.

This catches `AddNotes` IDs colliding with source notes, named-slot replacements reusing IDs outside the replaced slot, and arithmetic overflow from finite transpose operands. It runs for both legacy and explicit-order instances. The existing effective-ID encoding is unchanged: unusual delimiter-containing source/instance IDs can collide, and a full snapshot rejects those collisions rather than silently renaming events. A future collision-resistant encoding remains a separate compatibility decision.

Only emitted final notes are checked by this layer. Notes wholly outside a crop are omitted as before; a later whole-pattern replacement can remove transient duplicate IDs or overflowing pitch before output. Existing operation preconditions, source validation, budgets, ordering conflicts, and binding checks still run at their established earlier stages. Fractional finite pitch is not quantized or restricted to integer MIDI by this check, and intentional velocity clamping is preserved.

Errors use `InvalidProject` with the affected effective ID in the diagnostic message. Per-instance explanation and scoped dry-run share the same emitted-note validator; dry-run can report `executable:true` and `noteResolution.ready:false` because structurally valid unresolved edits remain saveable. Scoped dry-run checks only its affected first-loop results; cross-instance ID collisions and later-loop/global checks require full `project.snapshot`. Pending proposal snapshots are checked without changing main. Snapshot and HTTP render fail before invalid floating-point values can be serialized as JSON `null` or passed to the renderer; the HTTP result is 422, with no replacement state or successful WAV.

Canonical state is not normalized, silently deduplicated, or rewritten by failed resolution. Save/export/import may preserve structurally valid unresolved edits; undo or an explicit corrective command can recover them. The standalone audio renderer independently rejects duplicate effective IDs (`AudioError::InvalidValue`, field `id`) as well as its existing non-finite numeric input checks.

Finite musical values are not necessarily representable by the built-in DSP. A separate renderer boundary now requires finite positive tonal frequency, finite exercised phase/gain conversions and finite track/master samples, and the WAV encoder rejects non-finite samples before quantization. The core does not add an arbitrary MIDI-range clamp: a snapshot with extreme but finite pitch may remain structurally valid while rendering fails with HTTP 422 `AudioError`. `noteResolution.ready` is still not a full render guarantee.

The renderer rejects invalid arithmetic rather than sanitizing it to silence/full scale. Existing finite PCM clipping, fractional pitch, nominal envelope timing and representable source-end silence are preserved. Global input checks remain in effect; derived checks apply to exercised DSP paths, so muted/unused pitch-independent synthesis does not fail on an unused frequency calculation. See [audio numeric boundaries](audio.md#numeric-and-output-memory-boundaries) for exact scope.

The Rust output cap is 8,640,000 stereo frames, enforced before fallible buffer allocation. It preserves the HTTP 180-second/48-kHz profile; variable-rate library callers have the same frame budget, not the same duration. Host-side duration/source limits still return 413 `ResourceLimit`; subprocess render errors are wrapped as 422 `AudioError`. Failed renders preserve canonical HEAD and do not insert a render WAV/cache record. Supported correction/undo can restore a valid render; no source or command is silently rewritten.

## HTTP host (lead owns)

GET `/api/session` -> `{token}` per-process session (same-origin only).
GET `/api/projects` -> `{projects:[{id,name,revision,updatedAt}]}`.
POST `/api/rpc` body `{projectId?:string,method,params,idempotencyKey?:string}`; X-DAIW-Session header required for mutations. Returns worker response. Project creation omits projectId. GET `/api/projects/:id/export` returns canonical JSON download. POST `/api/projects/:id/import-audio` multipart upload handled by host (details coordinated).
GET `/api/projects/:id/render.wav?proposalId=...` returns revision-bound PCM WAV and X-DAIW-Revision. Rust `daiw-render` reads Snapshot JSON from stdin, writes WAV stdout; sample assets supplied by host using additional renderer input `assets:{[assetId]:{sampleRate,channels,samples:number[]}}` if supported. Browser clients can alternatively schedule Snapshot in their audio layer; all use the same effective snapshot.

## Ownership

Core worker: `crates/core/**`, domain/commands/proposals/resolver/CLI tests. Audio worker: `crates/audio/**`, `src/audio/**`, native backend tests and docs/audio.md. UI worker: `src/**` except `src/audio/**`, index.html, DESIGN.md. Lead: server, storage, integration, root manifests, scripts, desktop shell and project docs. No worker commits or changes `.dim`.
