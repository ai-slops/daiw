# daiw-core JSON-lines worker

`daiw-core` reads one request per stdin line and writes one response per stdout line. Every request has `{id,principal,state,method,params}`. Successful responses contain the new canonical `state`; failed responses never contain replacement state.

Build and start the worker:

```sh
sh scripts/rust.sh build
CORE=target/debug/daiw-core
```

One-shot commands are also available:

```sh
"$CORE" --help
"$CORE" fixture "CLI Demo" > project.json
"$CORE" validate < project.json
"$CORE" export < project.json > project.daiw.json
"$CORE" validate < project.daiw.json
"$CORE" explain instance-chorus-a < project.json
printf '%s' '{"state":PROJECT,"expectedRevision":"REVISION","command":{"type":"setTempo","tempo":120}}' | "$CORE" dry-run
```

The examples below use `jq` to keep an exported project in `$STATE`. IDs and revisions are strings, not array positions.

For worker requests, `$STATE` must be raw canonical Project (use `.content` from a native envelope). One-shot `validate`, `explain`, `export` and dry-run input accept native documents. `project.export` returns the v1 envelope; `project.import` accepts it under `params.project` and assigns fresh identity. See [native exchange and note selection](../../docs/native-exchange.md). `project.queryNotes` is a read-only query; `transformSelectedNotes` is also a supported typed command under normal dry-run/human-apply rules.

## Project methods

Create the default fixture:

```sh
printf '%s\n' '{"id":"1","principal":"human","state":null,"method":"project.create","params":{"name":"Demo","fixture":true}}' | "$CORE"
```

Import a trusted canonical project:

```sh
jq -nc --argjson project "$STATE" '{id:"2",principal:"human",state:null,method:"project.import",params:{project:$project}}' | "$CORE"
```

Read, validate, inspect history/hash, and render a snapshot:

```sh
jq -nc --argjson state "$STATE" '{id:"3",principal:"human",state:$state,method:"project.get",params:{}}' | "$CORE"
jq -nc --argjson state "$STATE" '{id:"4",principal:"human",state:$state,method:"project.validate",params:{}}' | "$CORE"
jq -nc --argjson state "$STATE" '{id:"5",principal:"human",state:$state,method:"project.history",params:{}}' | "$CORE"
jq -nc --argjson state "$STATE" '{id:"6",principal:"human",state:$state,method:"project.hash",params:{}}' | "$CORE"
jq -nc --argjson state "$STATE" '{id:"7",principal:"human",state:$state,method:"project.snapshot",params:{}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg proposalId "$PROPOSAL_ID" '{id:"8",principal:"human",state:$state,method:"project.snapshot",params:{proposalId:$proposalId}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg instanceId "$INSTANCE_ID" '{id:"9",principal:"human",state:$state,method:"project.explain",params:{instanceId:$instanceId}}' | "$CORE"
```

Restore a host-supplied canonical snapshot for undo or redo. The core verifies the project ID and advances from the current revision:

```sh
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --argjson snapshot "$OLD_STATE" '{id:"10",principal:"human",state:$state,method:"project.undo",params:{expectedRevision:$revision,snapshot:$snapshot}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --argjson snapshot "$REDO_STATE" '{id:"11",principal:"human",state:$state,method:"project.redo",params:{expectedRevision:$revision,snapshot:$snapshot}}' | "$CORE"
```

## Change methods

Dry-run and apply use the same typed command. Replace `$REVISION` with `state.revision`:

```sh
jq -nc --argjson state "$STATE" --arg revision "$REVISION" '{id:"12",principal:"agent",state:$state,method:"change.dryRun",params:{expectedRevision:$revision,command:{type:"setTempo",tempo:120}}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg revision "$REVISION" '{id:"13",principal:"human",state:$state,method:"change.apply",params:{expectedRevision:$revision,command:{type:"renameProject",name:"New name"}}}' | "$CORE"
```

Supported command tags are `setTempo`, `renameProject`, `addTrack`, `setTrack`, `addDefinition`, `instantiate`, `moveInstance`, `duplicateInstance`, `deleteInstance`, `editNotes`, `applyModifier`, `setModifierOrdering`, `rebaseModifier`, `removeModifier`, `createModifier`, `addAudioRegion`, and `removeAudioRegion`. Their fields are defined in `docs/contract.md`.

New reusable slot-modifier attachments retain source revision, slot ranges, source note IDs, and cardinality. Source drift requires an explicit human rebase; repeating `applyModifier` is not a refresh. Preview it first:

```sh
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --arg instanceId "$INSTANCE_ID" --arg modifierId "$MODIFIER_ID" '{id:"rebase-preview",principal:"agent",state:$state,method:"change.dryRun",params:{expectedRevision:$revision,command:{type:"rebaseModifier",instanceId:$instanceId,modifierId:$modifierId}}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --arg instanceId "$INSTANCE_ID" --arg modifierId "$MODIFIER_ID" '{id:"rebase-apply",principal:"human",state:$state,method:"change.apply",params:{expectedRevision:$revision,command:{type:"rebaseModifier",instanceId:$instanceId,modifierId:$modifierId}}}' | "$CORE"
```

The command changes binding metadata only. Legacy slot attachments remain readable but appear in `unboundModifierIds`; explicit rebase upgrades them. Only canonical `replaceSlot` selectors are supported by this strong-binding contract. See [binding semantics and downgrade limitations](../../docs/contract.md#persisted-strong-slot-modifier-bindings).

For explicit dependency ordering, use `setModifierOrdering` with `{instanceId,ordering:{edges:[{before,after}]}}`. For example, add-notes before transpose ensures the newly added notes are transposed too. Missing ordering preserves legacy array execution; an explicit empty edge list requires independent modifiers. `project.explain.modifierPlan` reports the actual order and core-derived read/write resources. New attachments can supply an optional complete `ordering` graph atomically through `applyModifier`. See [the ordering contract](../../docs/contract.md#explicit-modifier-dependency-ordering) for cycles, conflicts, removal behavior and downgrade restrictions.

## Proposal methods

Humans and agents may create proposals. Natural-language parsing is deliberately limited to the documented phrases; explicit operations are preferred:

```sh
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --arg instanceId "$INSTANCE_ID" '{id:"14",principal:"agent",state:$state,method:"proposal.create",params:{expectedRevision:$revision,instanceId:$instanceId,request:"octave up"}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --arg instanceId "$INSTANCE_ID" '{id:"15",principal:"agent",state:$state,method:"proposal.create",params:{expectedRevision:$revision,instanceId:$instanceId,request:"explicit",operations:[{type:"velocity",factor:0.8}]}}' | "$CORE"
```

Only humans may accept or reject. Acceptance reports each selected hunk as `Applied`, `Rebased`, or `RebaseConflict`; any blocking conflict preserves the supplied state:

```sh
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --arg proposalId "$PROPOSAL_ID" --arg hunkId "$HUNK_ID" '{id:"16",principal:"human",state:$state,method:"proposal.accept",params:{expectedRevision:$revision,proposalId:$proposalId,hunkIds:[$hunkId]}}' | "$CORE"
jq -nc --argjson state "$STATE" --arg revision "$REVISION" --arg proposalId "$PROPOSAL_ID" '{id:"17",principal:"human",state:$state,method:"proposal.reject",params:{expectedRevision:$revision,proposalId:$proposalId}}' | "$CORE"
```

## Error behavior

Agents cannot call `project.create`, `project.import`, `change.apply`, `proposal.accept`, `proposal.reject`, `project.undo`, or `project.redo`. Off-grid beat values, stale command revisions, invalid references, dependency gaps, and modifier conflicts return typed error codes. Diagnostics are emitted on stdout only as structured response fields; stderr is reserved for worker-level I/O failures.
