# Incremental source composition

The persistent JSON-lines core worker now reuses successfully composed source patterns across requests. The cache is owned by `WorkerRuntime`, not by a Project, the host database, global state, or an audio callback. Canonical full project state still comes from the host on every request. Restarting the worker discards all entries and counters without changing musical results.

This is the first incremental-resolution layer. A separate [instance-pattern cache](instance-resolution.md) now reuses post-modifier patterns. Neither layer caches instance repetition, final snapshots, track routing, DSP compilation or audio. Existing renderer-file caching is separate. Hashing dependencies, copying cached notes and checking bindings still incur work; cache hit counts are not a measured end-to-end speedup claim.

## Key and invalidation

A domain-separated BLAKE3 key includes project identity, root definition identity, and the actual reachable source-definition content:

- Definitions are ordered by ID and include their revisions and lengths.
- Note IDs, order, timing, pitch and velocity are included.
- Every child placement field is included, ordered by placement ID.
- Definition revisions remain in the key because nested provenance records those revisions.

Project revision/history/name, definition names/slots, instances, tracks, modifiers and proposals are excluded from this **base-composition** key. Their current state is still consulted by subsequent processing and validation. In particular, a cached base pattern does not skip current slot-binding, proposal, ordering, permission or stale-revision checks.

Changing a descendant produces new keys for dependent roots. Unrelated roots remain reusable; changing tempo or an instance override can reuse the source pattern while producing changed final output. Reordering children preserves a hit because composition is order-independent; reordering flat source notes changes the key. Same-revision content changes cannot return stale notes. Different project identities cannot share entries. Undo may reuse an earlier retained key, but cache state never determines undo history.

## Work and memory bounds

Entries contain a successful `ComposedNote` vector plus its original logical expansion cost. A hit charges that cost before copying the notes. Replay matches successive composer work charges, including zero-work patterns and meters that already exceeded the limit. Cached and uncached resolution therefore preserve the same output/error and work-budget behavior. Failed or partial compositions are never inserted; a successful base pattern may remain cached even if a later modifier or approval check fails.

The worker uses deterministic least-recently-used eviction with default aggregate limits:

| Bound | Default |
|---|---:|
| Entries | 64 |
| Accounted retained bytes | 33,554,432 (32 MiB) |
| Notes | 100,000 |
| Nested path entries | 500,000 |

Retained bytes count the backing capacity of the cache-entry vector, note/path vector capacities, and retained ID/string capacities. This excludes allocator metadata, stack values, temporary computation/copy buffers and OS RSS; it is not a process-memory cap. An individually oversized result is returned uncached without evicting useful entries. Allocation/accounting failures in cache admission leave the uncached result usable. Entries are never written to disk.

Public stateless library functions and one-shot CLI calls retain uncached behavior. `WorkerRuntime::with_cache_limits(CacheLimits::disabled())` supports differential library tests. There is no public RPC to clear, tune or bypass the cache.

## Read-only diagnostics

Authenticated human clients may call:

```json
{"projectId":"PROJECT_ID","method":"project.cacheStats","params":{}}
```

The result is `{enabled,limits,resident,totals,lastRequest}`:

- `limits`: `maxEntries`, `maxRetainedBytes`, `maxNotes`, `maxPathEntries`.
- `resident`: `entries`, `retainedBytes`, `notes`, `pathEntries`.
- Each counter group: `hits`, `misses`, `evictions`, `expandedWork`.
- `expandedWork` measures physical composer units on misses, including work before failed composition; it does not include key construction, cached copies or modifier execution.
- `lastRequest` describes the most recent ordinary worker request, including failures. Non-composition methods can replace it with zeros.

Despite its project-prefixed method name, these are **worker-wide aggregate** diagnostics. No cache keys, names or musical payloads are returned. Reading stats does not touch LRU order/counters, replace `lastRequest`, advance HEAD, or create journal/idempotency entries. Both core and host deny this diagnostic query to agents. Telemetry is absent from canonical state, native exports and render snapshots.

## Verification and remaining work

Differential tests compare full disabled/cold/warm responses for snapshots, explanations, note queries, dry-runs and stale guards. Additional tests cover same-revision changes, nested provenance, child ordering, flat note ordering, cross-project isolation, exact work errors, aggregate eviction, oversized admission and mixed-size replacement after storage compaction.

Live HTTP checks observed a nested fixture's warm snapshot using six hits with zero physical expansion. Editing a child produced two misses while four unrelated/repeated roots reused cached results. Restart reset residency; the snapshot and freshly regenerated WAV were identical to their pre-restart versions. See [verification.md](verification.md) for counts and evidence.

Post-modifier per-instance pattern reuse is now implemented separately. Final-event/snapshot reuse, sub-definition incremental compilation, signal scheduling, disk streaming, long-session profiling and native-device qualification remain outstanding. Neither cache requires a new native format feature because persisted musical meaning is unchanged.
