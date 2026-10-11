# Agent chat backend

The Node host provides an owner-scoped, durable chat backend for the React client. Chat state and credentials are stored separately from canonical projects. Rust remains the only musical-domain authority, and model tools can inspect, dry-run, or create an unapproved proposal only. They cannot apply, accept, reject, access a shell or filesystem, or replace project state.

## Using the Assistant

Open **Assistant**, choose **Continue with ChatGPT**, and approve ChatGPT plan access. A local browser returns to the host's loopback listener automatically. On a remote preview, the final loopback page may fail to connect on your computer: copy that complete address into **OAuth callback URL** in DAIW and choose **Complete sign-in**. Treat the callback address as sensitive; never paste it into a chat message or issue report.

Choose an available model and **New chat**, then explicitly approve sending this project's context before sending a message. The transcript shows text and tool progress. **Cancel response** stops subsequent model/tool work; an already committed proposal remains available for human review. Open **Proposal review** to inspect and approve changes through the existing human-only controls. No raw audio is uploaded. Account/project changes reset context consent, and signing out clears the displayed chat. Stored sessions can be reopened; interrupted turns are reported rather than silently rerun.

The current workflow is a bounded model/tool loop plus durable proposal review, not a general-purpose workflow graph builder. SDKs execute only in the host; the React UI uses `@assistant-ui/react` with an external-store adapter. The application does not use Codex/OpenCode runtimes or a separate workflow framework.

## Browser ownership and request security

`GET /api/chat/auth/status` creates a random one-year `daiw_chat_sid` cookie when needed. The cookie is `HttpOnly`, `SameSite=Strict`, and persisted only as a SHA-256 ownership key in the protected chat database. It is unrelated to `/api/session` and no GUI token is reused.

Every chat `POST` requires the cookie and an `Origin` exactly matching the effective request origin. Requests without `Origin` fail. Hostname checks and trusted loopback proxy handling run before chat routing. Sensitive `GET` routes also require ownership. Non-2xx chat responses have one shape:

```json
{"error":{"code":"StableCode","message":"Human-readable message"}}
```

## Frozen HTTP contract

- `GET /api/chat/auth/status` -> `{hostId,callbackMode,accounts,activeAccountId}`.
- `POST /api/chat/auth/login` with `{accountId?,mode?}` -> `{loginId,authorizationUrl,redirectUri,completionMode,expiresAt}`.
- `GET /api/chat/auth/login/:loginId` -> `{status:"pending"|"completed"|"failed"|"expired",activeAccountId?,error?}`.
- `POST /api/chat/auth/login/complete` with `{loginId,callbackUrl}` -> `{activeAccountId}`.
- `POST /api/chat/auth/logout` with `{accountId}` -> `{revocationConfirmed}`.
- `POST /api/chat/auth/select` with `{accountId}` -> `{activeAccountId}`.
- `GET /api/chat/models?accountId=...` -> `{models:[{id,displayName}]}`.
- `GET /api/chat/sessions?projectId=...&accountId=...` -> `{sessions:[...]}`.
- `POST /api/chat/sessions` with `{projectId,accountId,model}` -> `{session}`.
- `GET /api/chat/sessions/:id` -> `{session,turns}`.
- `POST /api/chat/sessions/:id/turns` with `{clientTurnId,text}` -> `{turnId,eventsUrl}`, HTTP 202.
- `GET /api/chat/sessions/:id/turns/:turnId/events` streams durable SSE. Resume with `Last-Event-ID` or `?after=`.
- `POST /api/chat/sessions/:id/turns/:turnId/cancel` -> `{}`.

One turn may run per session. Repeating a `clientTurnId` returns the original turn. A second distinct turn while one is running returns HTTP 409. Event names and data are:

- `text.delta`: `{text}`
- `tool.started`: `{callId,name}`
- `tool.completed`: `{callId,name,result}`
- `turn.completed`, `turn.failed`, `turn.cancelled`, or `turn.interrupted`: `{turnId,error?}`

Events have increasing integer SSE IDs and are persisted before emission. Each returned turn includes its integer `lastEventId`; reconnect with that value to avoid replaying already-applied events. Terminal state and its event are committed atomically. A host restart marks unfinished turns `interrupted` and creates a durable terminal event; it never resumes side effects. Partial display text is durable, while each completed provider response and tool output is appended incrementally to provider history.

## Sign in with ChatGPT

The implementation follows the current OpenAI OSS token-sharing documents for registration/sign-in, profiles/sessions, models/inference, and preview limitations. It uses fixed OpenAI authorization, token, discovery, JWKS, and API origins. There is no configurable production provider endpoint or credential environment-variable fallback.

Initial registration uses `dynamic_agent_client`, `agent_name_hint=DAIW`, and a stable `urn:uuid` host ID. Returning sign-in reuses the issued client ID and verified subject and may include an email `login_hint`; authorization URLs never include a retained ID token. Every attempt has fresh 32-byte state, nonce, and PKCE verifier, a ten-minute single-use record, and an exact `http://127.0.0.1:<port>/auth/callback` URI. The callback mode is fixed by the requesting origin: direct loopback HTTP uses automatic completion, while an HTTPS remote preview uses paste completion. A client cannot override that choice. Paste completion parses the copied URL but never fetches it. Auto completion uses a temporary loopback listener and returns a no-store, no-referrer, restrictive-CSP result page before closing it. Clients poll the owner-scoped login-status route for the terminal result.

ID tokens are verified with `jose` against OpenAI JWKS, fixed issuer `https://auth.openai.com`, issued-client audience and authorized party, nonce, required expiration/issued-at/subject claims, and RS256. Both `resource.invoke` and `chatgpt.tokens.use.direct` scopes are required. Credentials are stored in `${XDG_CONFIG_HOME:-~/.config}/daiw/assistant/chat.sqlite` by default. The storage path must be absolute and may not traverse symlinks; its directory is `0700`, and the database plus existing WAL/SHM sidecars are `0600`. SQLite WAL/FULL transactions provide atomic records. Browser responses never contain access, refresh, or ID tokens, subjects, or issued client IDs. Accounts and retained dynamic registrations are scoped to the browser-owner cookie, so clearing that cookie does not expose them to the replacement browser identity. Do not log authorization URLs.

Refreshes and logout are serialized per account. Rotated token sets are persisted together before a refreshed identity or scope validation error is surfaced, but quarantined as disconnected so they cannot be used for inference and the UI requires reauthentication; provider `earliest_refresh_at` is honored while the current access token remains valid. Logout cancels owned turns, waits for an in-flight refresh, attempts revocation with the newest credentials using the fixed OpenID discovery document, and clears local tokens in `finally`. `revocationConfirmed:false` means local logout succeeded but remote revocation was not confirmed.

## Inference and tools

The official `openai` SDK is constructed per account operation with its OAuth access token, retries disabled, a 120-second timeout, and logging off. Every Responses request uses `store:false`, `stream:true`, full ordered input history, and a `DAIW` namespace. It omits `previous_response_id`, `conversation`, `background`, `metadata`, `temperature`, and output-token controls required to be absent in this preview.

Only these strict tools exist: `project_context`, `project_query_notes`, `project_explain`, `change_dry_run`, and `proposal_create`. Project/account/principal identity and the captured project revision are host-injected. Calls run through `ProjectService.execute(..., "agent")`, so the existing Node and Rust allowlists remain authoritative. Every call checks the turn's captured revision; a later project head fails with `StaleRevision` instead of silently rebasing. Tool results are capped at 64 KiB and exclude assets, credentials, and full project payloads. Call IDs and hashes persist for deterministic replay.

A turn accepts at most 16 KiB of user text, 256 KiB of retained history, eight tool calls, six model iterations, and 120 seconds. Old complete turn groups are removed as groups when history exceeds the cap. Cancellation is checked before provider requests and tools. A proposal already committing at cancellation may finish and emit `tool.completed`, but remains unapproved and cannot alter the audible main arrangement.

The host sends no project content in the initial model request. Project context is exposed only after the user explicitly sends a turn and the model invokes an allowed tool. The frontend must make that send action and external-provider disclosure explicit; application/project content is untrusted data, never a developer instruction.

## Provider fixture and verification scope

Integration tests inject a provider object through `ChatService`'s constructor only. Production has no environment-controlled endpoint or credential bypass. Tests exercise real loopback HTTP routing and protected SQLite persistence, plus a fake OAuth token endpoint, signed RSA JWKS fixture, model catalog, and streamed Responses events. Covered failures include wrong audience, nonce, state, expired ID token/login, ownership isolation, refresh rotation, logout, one-active-turn conflict, cancellation/SSE replay, restart interruption, stale revisions, and forbidden tool arguments.

The user completed real ChatGPT authorization. A live-account diagnostic then verified a text response and an isolated tool loop through the production provider/runner, returning `OK` with ordered `function_call`, `function_call_output`, and message history. No user-project content was sent by that diagnostic. Streamed `response.output_item.done` items are retained because the terminal response can omit its output array; dropping those call items causes later tool-result replay to fail with HTTP 400. Live refresh/revocation and all account/model combinations are not certified by this smoke test.

SIWC is a public preview: model/tool support and account/workspace policy remain provider-controlled. Remote HTTPS DIM previews use paste mode: copy the final loopback callback URL after the browser cannot connect locally, submit it to the completion route, and poll the login-status route. The host verifies the URL without requesting it. This is a protocol accommodation, not a policy, consent, usage-limit, or account-access bypass.

## Frontend chat-flow verification

Build the Rust core and frontend, then run the signed local OAuth/provider browser flow:

```sh
npm run core:build
npm run build
npm run test:chat-flow
```

The flow uses Playwright's bundled Chromium by default. Set `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` to use a specific Chromium executable and `QA_EVIDENCE_DIR` to change the screenshot/results directory.
