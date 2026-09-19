# DAIW

A structured digital audio workstation built around shared **Definitions**, placed **Instances**, composable **Modifiers**, and human-reviewed editing proposals.

## Implementation target

This repository targets stages 0 and 1 of the supplied design: a headless Rust domain engine and a first desktop-oriented vertical slice. Recording, comping, third-party plugins, collaboration, and interchange beyond native projects belong to stages 2–4. See [the roadmap](docs/roadmap.md) for the later work and [the integration contract](docs/contract.md) for current interfaces.

The `.dim/` lifecycle configuration is preserved. Its `node app/server.mjs` entrypoint hosts the client and delegates canonical project operations to Rust. SQLite stores immutable snapshots, a command journal, and the current head in one transaction. GUI state is never the canonical project.

**Coverage is not full stage-1 sign-off.** This is an executable structural/audio/review vertical slice. Full-session disk streaming, the complete nested composition/selector graph, and physical Windows/Linux desktop/audio qualification remain open. The implementation and verification ledger distinguishes working behavior from the original design's remaining requirements.

## Development

Requirements: Node.js 24+, Rust 1.94+ with a C linker, or Docker for the isolated Rust build helper.

```sh
npm ci
npm run core:build
npm run build
npm start
```

Open `http://localhost:3000`. For client hot reload, also run `npm run dev` and open port 5173. The API remains on port 3000.

On first DIM startup the entrypoint installs missing dependencies and builds missing artifacts, then registers the preview URL. It does not alter `.dim`. Subsequent starts reuse built artifacts; rebuild after editing source.

For the desktop shell, run `npm run desktop` after building. On Linux this requires Electron's normal desktop shared libraries and a display server. `npm ci --ignore-scripts` skips Electron's binary download; run `node node_modules/electron/install.js` when using that installation mode.

```sh
npm run core:test
npm run test:server
npm run build
```

`scripts/rust.sh` uses a local compiler when available; otherwise it copies only Rust sources into an isolated Docker build container. The container does not require host bind mounts, which are unavailable in DIM's rootless DinD environment.

## Design invariants

- Rust owns typed commands, domain validation, resolution, and approval.
- Shared-source edits and instance-local edits are explicit.
- Dry-run does not mutate state. Unapproved proposals do not change the audible main arrangement.
- Stale revisions and conflicting operations fail explicitly.
- Original media are immutable content-addressed blobs; decoded audio and renders are rebuildable caches.
- Agent credentials do not grant approval or main-head mutation rights.
- No project material is sent to an external model by default.

All documentation is written in English. See [architecture](docs/architecture.md), [API](docs/api.md), and [verification](docs/verification.md) for supported behavior and validation limits.
