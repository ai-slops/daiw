# Architecture

## ADR-0001: Structured projects and reviewable changes

**Status:** accepted for the stage-0/1 implementation.

The canonical project is a semantic graph, not a React component tree. Definitions contain reusable musical content. Instances place that content on tracks and the timeline. Modifiers describe reusable differences; instance overrides describe differences local to one placement. Editing a definition changes its dependents. Editing an instance must not silently detach or mutate its source.

Every public mutation is a typed Rust command. Resolution generates effective events with origin information. Conflicting slot replacements or parameter writes fail explicitly. A proposal is stored separately from the musical main state, and acceptance constructs a fresh validated revision from selected semantic hunks. Agent principals cannot approve proposals or issue ordinary main mutations.

## ADR-0002: DIM host and Rust authority

**Status:** accepted.

DIM requires `node app/server.mjs`; `.dim` remains unchanged. Node serves the web client, authenticates callers, owns SQLite transactions, and manages worker processes. A Rust worker receives a canonical snapshot and a typed operation over JSON lines. It returns a new canonical snapshot or a structured error. Node never patches domain fields.

This small split deliberately avoids making browser state, Node route handlers, or an ORM a second domain authority. Full snapshots are used instead of replaying all historic commands. The database is authoritative after a crash; workers can be restarted from a stored snapshot.

## ADR-0003: Transactional snapshot persistence

**Status:** accepted.

`data/state.sqlite` uses WAL and synchronous FULL. `revisions` holds immutable payloads and SHA-256 integrity hashes. `projects.head` references the current revision. `journal` records request keys, hashes, principals, base revisions, resulting revisions, and responses. `undo_stack` stores durable undo/redo revision references.

The host serializes command execution, then uses a SQLite compare-and-swap transaction. A stale base cannot update HEAD. Revision insertion, journal insertion, undo history, and HEAD move commit together. A repeated idempotency key replays the saved response only when its request hash matches. A crash before commit leaves the previous head; a crash after commit preserves the response for retry.

There is no automatic migration from an unknown schema. Future migrations must create a verified SQLite backup before changes, as described in the roadmap.

## ADR-0004: Immutable media and rebuildable caches

**Status:** accepted.

Original audio is stored at `data/objects/<sha256>`. Blob writes use a temporary file, flush, and atomic rename. SQLite stores asset metadata; project regions refer to stable asset IDs, never original file paths. Decoded PCM lives in `data/cache/audio`; WAV previews in `data/cache/render`. Deleting caches must not destroy a project. Missing or corrupted originals are explicit errors, never silence.

On POSIX systems the containing directory is synced after rename. Windows uses flushed files and atomic rename, but equivalent power-loss directory-entry durability has not been verified. Process-crash recovery is tested separately from hardware/power-loss durability.

Original-media hashing is SHA-256 in this host; canonical musical fingerprints use the Rust domain's hashing policy. Algorithms are kept distinct rather than pretending hashes are interchangeable. This is an explicit change from the design document's recommended all-BLAKE3 storage choice, made to use Node's standard-library integrity implementation at the storage boundary.

Render keys include the effective snapshot, source PCM, and renderer profile/version. The renderer consumes the same effective notes used by explanation and editing views. Node does not synthesize audio.

## ADR-0005: Desktop shell and audio boundaries

**Status:** accepted with platform-verification limits.

The primary client is React/TypeScript, served locally and usable in Chromium or an Electron shell. Electron uses sandboxing, context isolation, and no renderer Node integration. Rust audio is independently testable and usable headlessly. Browser playback consumes Rust-rendered audio; the native Rust backend is a separate adapter, not a claim that browser scheduling is a native hard-real-time callback.

Native output must be tested on actual Windows and Linux devices before claiming production latency and driver compatibility. CI compilation and deterministic offline audio tests cannot establish those properties. See `audio.md` and `verification.md` for what has actually been exercised.

## Trust boundaries

- Same-origin GUI sessions carry a random per-process session token.
- Optional agent clients use a separate bearer token and a restricted endpoint.
- Rust checks authorization again; client-supplied author names never grant privileges.
- Uploads have size/duration limits and decode as data, never executable code.
- Raw snapshot replacement is not exposed as a normal edit command. Native project import is a separate validation path.
- No plugin execution or network model calls are enabled by default.
- User project text is content, not privileged system instructions.

## Repository boundaries

| Path | Responsibility |
|---|---|
| `crates/core` | Typed model, commands, resolution, conflicts, proposals, CLI |
| `crates/audio` | Immutable playback input, DSP, media decoding, WAV, native adapter |
| `app` | HTTP, process transport, transactional storage, immutable media |
| `src` | Client views and ephemeral selection/transport state |
| `src/audio` | Browser audition adapter |
| `desktop` | Sandboxed Chromium desktop lifecycle |
| `tests` | Host persistence and HTTP integration checks |
| `.dim` | Preserved environment lifecycle |
