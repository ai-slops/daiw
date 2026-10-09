# Native exchange and bounded selections

## Version 1 exchange

`project.export` and HTTP project export produce `{format:"daiw-project",formatVersion:1,requiredFeatures,contentHash,content}`. `content` holds the canonical Project, including its revision and history. SQLite snapshots and worker `state` remain raw Project objects; they are not envelopes. Import creates a new project ID and revision after validation; it never overwrites the original.

Required features are derived from content and must match exactly in sorted, unique order: `core-v1`, and, when used, `audio-regions-v1`, `modifier-ordering-v1`, `named-slots-v1`, `note-patches-v1`, `proposal-review-v1`, `slot-bindings-v1`. Explicit-empty modifier ordering still requires its feature. Unknown format/version/features or semantic fields fail closed. Documents containing reserved envelope keys never fall back to legacy parsing after an envelope error. Legacy raw documents remain accepted with `sourceFormatVersion:0`.

`contentHash` is BLAKE3 over recursively key-sorted JSON of the **typed canonical Project**, not the literal input document and not the revision-independent project hash. Defaults are normalized before hashing: omitting a default-empty field can preserve the hash. Signed zero normalizes to positive zero; finite float parsing preserves binary round trips. Integer metadata outside ±(2^53−1) is rejected for portable JSON precision. This detects canonical-content changes, not byte-level alterations or authenticity. Future schema changes must preserve this v1 normalization or introduce explicit version conversion. The format does not provide signatures, a migration framework, or media archives.

Imported audio references must match local asset hash, frames, channels and sample rate before reuse. Missing media is not bundled or synthesized. Decoded caches remain rebuildable local files; their sample payloads are not authenticated independently. A locally corrupted but structurally valid decoded cache can affect playback and should be removed for regeneration. Cache authentication is an outstanding storage-hardening task.

## Revision-bound note handles

Nested definitions and their descendant revision bindings require `nested-definitions-v1`. See [composition.md](composition.md). Flat projects keep their previous features and canonical-content hash.

Timing maps additionally require `tempo-map-v1` and `meter-map-v1` respectively. Their absence leaves existing native hashes unchanged; see [timing.md](timing.md).

`project.queryNotes` is read-only for humans and agents. It resolves one nonrecursive selector in canonical source notes or instance-effective editable first-loop notes. Results sort by onset then stable ID. Required `maxResults` is 1–4096; over-limit queries fail rather than truncate. Supported selectors and transform wire shapes are listed in [api.md](api.md#bounded-note-selection).

The handle includes project ID/base revision, definition ID/revision, instance ID, edit scope, selector, ordered note IDs, cardinality and resolution fingerprint. Apply checks these values and resolves the selector again. The fingerprint is an unkeyed consistency check, **not a capability or authorization token**. Any canonical project revision change conservatively invalidates the handle. No adaptive rebasing occurs.

`transformSelectedNotes` supports finite transposition and normalized velocity setting under the existing human-only expected-revision boundary. Empty and semantically unchanged transformations fail without mutation, preventing spurious source revision changes from invalidating strong slot bindings.

Instance transformations use the existing stable-ID whole-note patch representation. Changing velocity captures the selected effective note's other fields too: removing an earlier transpose afterward does not restore that note's source pitch. Untouched notes still inherit source edits. This is deliberately not field-level live composition. Handles are ephemeral, so they add no persisted native feature. Recursive selectors, Adaptive/Snapshot selection modes, capability issuance, nested graph selection and dedicated selector UI remain unimplemented.
