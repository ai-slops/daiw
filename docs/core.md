# Core worker and CLI

`daiw-core` is the canonical stateless domain worker. With no arguments it reads newline-delimited request objects from stdin and writes exactly one response object per line to stdout:

```sh
printf '%s\n' '{"id":"create-1","principal":"human","state":null,"method":"project.create","params":{"fixture":true}}' | target/debug/daiw-core
```

A successful line is `{id,ok:true,state,result}`. A failed line is `{id,ok:false,error:{code,message}}`; failures never provide replacement state. Structured protocol data is written only to stdout. stderr is reserved for malformed process I/O or fatal worker diagnostics.

The worker recognizes project create/import/get/validate/snapshot/explain/hash/history/undo/redo, change dry-run/apply, and proposal create/accept/reject. `project.undo` and `project.redo` require `{expectedRevision,snapshot}`; the host supplies the opaque durable snapshot after checking its history stack.

One-shot CLI commands use the same library authority:

```sh
target/debug/daiw-core --help
target/debug/daiw-core fixture "CLI Demo" > project.json
target/debug/daiw-core validate < project.json
target/debug/daiw-core explain instance-chorus-a < project.json
jq -n --slurpfile state project.json --arg revision "$(jq -r .revision project.json)" '{state:$state[0],expectedRevision:$revision,command:{type:"setTempo",tempo:120}}' | target/debug/daiw-core dry-run
```

`fixture` emits canonical Project JSON. `validate` emits `{valid,hash}`. `explain` emits the selected instance, its `instanceId`, source definition, modifier chain, and local overrides. `dry-run` emits executability, affected instance IDs, command hunks, and diagnostics without changing its input.

Entity IDs and project revisions are opaque strings; definition revisions are integer counters. Beat values must lie exactly on the 1/960-beat grid. Imported projects receive a fresh Project ID and revision. Canonical hashes exclude revision/history metadata; project revisions advance uniquely for every accepted canonical mutation, including proposal creation and rejection.
