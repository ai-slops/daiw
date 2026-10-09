# Incremental instance-pattern resolution

The persistent worker now has two derived caches: [source composition](incremental-resolution.md) and post-modifier instance patterns. Both are process-local, bounded, noncanonical, and empty after worker restart. No native format feature or persisted project field is added.

## Reuse boundary

The second cache stores the complete first source pattern after ordered reusable modifiers and instance-local overrides, including source identity, nested paths and modifier provenance. It does not store placed/repeated effective events or the cropped editable window.

Every resolution still checks current modifier ordering, strong bindings and operation validity before lookup. Snapshot processing preserves its prior order: validate operations, derive/check the effective track, calculate the remaining event budget, resolve/reuse the pattern, then repeat/crop and validate emitted notes. A cache hit is not an approval or a replacement for current guards.

Instance placement, length, outer repetition, track assignment, tempo and final event validation remain live. Moving an instance therefore reuses its pattern while updating note positions. Shortening it recomputes cropping; growing it recomputes repetitions and event limits. Proposal-preview operations use distinct pattern keys from ordinary main output and retain current proposal assessment.

## Key and invalidation

A separate domain-separated BLAKE3 key includes:

- The source-composition dependency digest, including project/root identity, actual reachable source content and provenance revisions.
- The logical instance ID, so duplicate instances start independently cold.
- All root named-slot names and boundaries.
- The actual ordered resolved operations, including modifier ID presence/value, operation tags, float bits, ticks, note IDs and ordered payloads.

Changing a source, modifier payload/order, slot boundary or local patch cannot return an old pattern even when supplied revisions are unchanged. Placement, length, track and tempo are deliberately excluded because they are applied downstream. Changing binding metadata cannot bypass validation just because it is excluded from the pattern key.

## Logical work and admission

Modifier processing records the exact ordered work-charge trace, including zero-sized charges. A hit replays each charge through the existing `WorkMeter` before cloning retained notes. This preserves the same failure point, message and meter state for mid-sequence exhaustion or an already-exceeded meter; an aggregate total would not be equivalent.

Only successful transformations are admitted. Final validation remains downstream: a temporarily invalid intermediate value may legitimately be removed by a later replacement, and an invalid final result is never made valid by caching. Failed transformations are not cached. A transformed pattern may remain resident when a later placement/output check rejects it; that later check runs again on reuse.

Cold/disabled processing moves composed note payloads instead of deep-copying them. Known-oversized candidates are screened by note/path/minimum-byte counts before making a retained copy, followed by actual-capacity admission checks. The source and instance caches share the same tested deterministic LRU storage implementation.

Each cache independently defaults to 64 entries, 32 MiB accounted retained bytes, 100,000 notes and 500,000 definition-path entries. Together the defaults permit at most **128 entries, 64 MiB, 200,000 notes and 1,000,000 definition-path entries**. Modifier-ID storage and the work-trace vector count toward retained bytes. As with the source cache, allocator metadata, stack values, transient copies and process RSS are excluded; this is not a process-memory cap.

## Diagnostics

`project.cacheStats` retains its original source-only shape and meaning. Human clients can additionally request:

```json
{"projectId":"PROJECT_ID","method":"project.instanceCacheStats","params":{}}
```

It returns the same `{enabled,limits,resident,totals,lastRequest}` schema, scoped to the worker's instance-pattern cache. `expandedWork` is the sum of operation-work charges attempted on misses, including a charge that fails its budget check; it excludes source composition, key construction, copies, placement, validation and DSP. Warm hits replay logical charges but add no new `expandedWork`.

Reading either diagnostics method does not reset either cache's counters or LRU state, change HEAD, or create journal/idempotency records. Both are denied to agents by host and core. An ordinary worker request resets both `lastRequest` groups, even when it does no cache work. Public stateless APIs remain uncached; the library limit constructor disables both caches when passed disabled limits.

## Verified scope and exclusions

Tests compare disabled/cold/warm responses for snapshots, explanations, queries, dry-runs and proposal previews. They cover current guard failures, operation/slot/source changes, movement/cropping, provenance, exact trace replay, transient-invalid cleanup, retained-memory bounds and oversized admission. Host scenarios verify repeated local edits invalidate only the changed instance while source edits invalidate both direct and nested dependents.

A live HTTP scenario observed six warm hits, no new modifier work, unchanged results; moving a clip kept all six hits while shifting its notes. A local transpose then caused one miss and five hits. Restart cleared both caches, and regenerating the WAV after removing render files produced identical bytes.

This does not supply incremental DSP compilation, signal scheduling, final-event fragment reuse or disk streaming. Key construction and copying still cost time; hit telemetry is not a measured end-to-end speedup or long-session performance certification. No UI surface changed in this batch.
