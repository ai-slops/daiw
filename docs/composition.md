# Nested definition composition

Definitions can reuse other definitions through stable child placements. Top-level arrangement instances still own track routing, modifier order and instance-local overrides. This is bounded note composition, not a full part/lane, signal-routing or arrangement-flow graph.

## Canonical shape and commands

```json
{
  "id": "parent",
  "name": "Phrase",
  "length": 8,
  "notes": [],
  "revision": 1,
  "children": [
    {"id": "placement-id", "definitionId": "riff", "start": 2, "length": 4}
  ]
}
```

`children` is omitted when empty. Each placement has a project-global unique ID, a referenced definition, and a half-open parent-local range. Start is nonnegative, length positive, and the entire placement must fit the parent. Coordinates use the existing 1/960-beat grid.

Human-only commands with `expectedRevision`:

- `addDefinitionChild {parentDefinitionId,definitionId,start,length}` allocates a stable placement ID.
- `setDefinitionChild {parentDefinitionId,child:{id,definitionId,start,length}}` changes an existing placement without replacing its ID.
- `removeDefinitionChild {parentDefinitionId,childId}` removes that reference, not its source definition.
- `editDefinitionNotes {definitionId,notes}` replaces only that definition's own notes, including when it has no top-level instance.

Only the edited definition's revision advances; revisions do not cascade through ancestors. New exact no-op setters return `InvalidParams`. Legacy `editNotes(scope="source")` remains accepted for identical notes but no longer increments the definition revision in that case. Both source-edit paths check revision overflow instead of wrapping. Invalid commands do not commit replacement state.

## Resolution and identity

The resolver combines parent-owned notes with recursively expanded children. A child repeats at its definition length inside the placement; onsets outside the placement are excluded and tails crossing its end are cropped. Top-level modifiers and then local overrides act on the combined parent pattern, which is finally repeated/cropped into the arrangement instance. All resulting notes inherit the top-level instance track, including any derived strip.

Nested pattern IDs hash a domain-separated structured identity containing root definition, placement/child-loop path, leaf definition and leaf note. Revisions, pitch, velocity and placement timing are excluded: source edits and placement moves preserve identities. Different placements and repetitions remain distinct. Repointing or recreating a placement changes its descendant identities. Nested-root effective event IDs separately hash the top-level instance, outer repetition and pattern ID. Flat effective IDs and flat native hashes remain unchanged.

`project.explain` adds `compositionNotes`, the pre-modifier parent-local pattern with scoped IDs and origins. Existing `effectiveNotes` remains the editable first period, cropped to the smaller of definition and instance length. A nested origin includes `definitionPath` entries with definition/revision, placement ID and child-loop index; emitted events additionally carry `instanceLoopIndex`. Parent-owned notes retain their own canonical identity.

## Editing and dependency guards

Source editing and source selection see only the selected definition's own notes. They never flatten descendants into that source. Instance editing/selection sees the combined effective first-period notes; stable-ID patches can alter one nested occurrence without changing its source or peer instances. Untouched nested notes inherit later source edits. Deleting, cropping out or repointing a patched target can produce `RebaseConflict`; the system does not silently retarget it. Patches still own entire note records, not individual fields.

Strong slot bindings use the composed pre-modifier pattern and a sorted `definitionRevisions` closure. Any reachable descendant revision change conservatively invalidates a captured nested binding, even outside the selected slot. Missing nested closure metadata is also stale. Explicit `rebaseModifier` refreshes metadata rather than silently rewriting the music. Proposal anchors/slot bindings use the same descendant guard. Pending proposal previews now perform rebase assessment before applying hunks, so stale nested proposals cannot be auditioned as if still valid. Create/review a new proposal after a conflicting source change; no general proposal-rebase editor is supplied.

Dry-run impact follows transitive parents and unions reachable definition dependencies from both before and after states. Repointing A to B reports both dependencies. Structural `executable` and note-resolution readiness remain distinct: a source change may be saveable while making a local patch unresolved.

## Bounds and compatibility

- All definitions, including uninstantiated ones, are checked for missing references and cycles.
- Maximum depth: 16 placement steps. Maximum placements: 4096 per project.
- Composition and per-note operation work use a 100,000-unit request-scoped meter on resolution paths; repeated empty-child traversal is charged too. Bound modifiers reuse the composed pattern instead of expanding it again.
- Pattern growth is capped before extension. Snapshot output is capped at 100,000 actually emitted notes; cropped-away events are not charged as emitted output, and empty patterns return immediately.
- Structural validation does not assert that every possible expansion is renderable. Over-budget resolution fails explicitly rather than returning partial music.
- Native format v1 adds `nested-definitions-v1`. Empty children normalize to omission without changing the flat content hash. Older readers must reject the feature rather than discard child references.

## Editor workflow and remaining scope

The Inspector's **Composition** section selects any parent definition, adds/edits/removes placements, and navigates child sources using breadcrumbs. Failed saves retain the draft and show an inline error; corrected retry and cancel clear it. Project switches reset the definition path.

The Source piano roll edits own notes only. For the selected arrangement root, nested notes appear as read-only ghosts with provenance. This-instance mode edits the composed effective notes. Navigating another definition edits its own notes without creating a hidden arrangement instance. When that definition has children but no matching explanation, the UI explicitly says the composed preview is unavailable rather than suggesting the source has no nested content.

Child-level modifier chains, part/parameter bindings, nested audio regions, independent nested routing, time transforms, structural extraction/squashing, graphical hierarchy editing, adaptive selectors and incremental compilation remain outside this implementation.
