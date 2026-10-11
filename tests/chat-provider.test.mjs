import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { generateKeyPair, exportJWK, SignJWT } from "jose";
import { test } from "node:test";
import { OpenAIProvider } from "../app/chat/provider.mjs";
import { ChatService } from "../app/chat/service.mjs";

async function providerFixture(overrides = {}, tokenOverrides = {}) {
	const claims = { algorithm: "RS256", issuer: "https://auth.openai.com", audience: "oaiapp_fixture", nonce: "nonce-1", subject: "subject-1", expiration: "future", issuedAt: true, ...overrides };
	const { privateKey, publicKey } = await generateKeyPair(claims.algorithm);
	const publicJwk = { ...(await exportJWK(publicKey)), kid: "fixture-key", alg: claims.algorithm, use: "sig" };
	const now = Math.floor(Date.now() / 1000);
	let signer = new SignJWT({ nonce: claims.nonce, email: "fixture@example.com", ...(claims.azp === undefined ? {} : { azp: claims.azp }) })
		.setProtectedHeader({ alg: claims.algorithm, kid: "fixture-key" })
		.setIssuer(claims.issuer)
		.setAudience(claims.audience)
		.setSubject(claims.subject);
	if (claims.issuedAt) signer = signer.setIssuedAt(now);
	if (claims.expiration !== undefined) signer = signer.setExpirationTime(claims.expiration === "future" ? now + 300 : now - 1);
	const token = await signer.sign(privateKey);
	const calls = [];
	const revokedTokens = [];
	const fetchImpl = async (input, init = {}) => {
		const url = String(input);
		calls.push(url);
		if (url.endsWith("/.well-known/jwks.json")) return Response.json({ keys: [publicJwk] });
		if (url.endsWith("/api/accounts/oauth/token")) return Response.json({ access_token: "access", refresh_token: "refresh", id_token: token, token_type: "Bearer", expires_in: 3600, earliest_refresh_at: now - 1, scope: "openid resource.invoke chatgpt.tokens.use.direct", ...tokenOverrides });
		if (url.endsWith("/.well-known/openid-configuration")) return Response.json({ issuer: "https://auth.openai.com", revocation_endpoint: "https://auth.openai.com/oauth/revoke" });
		if (url.endsWith("/oauth/revoke")) { revokedTokens.push(new URLSearchParams(init.body).get("token")); return new Response(null, { status: 200 }); }
		throw new Error(`unexpected URL ${url}`);
	};
	return { provider: new OpenAIProvider({ fetchImpl }), calls, revokedTokens };
}

test("provider verifies a signed OpenAI ID token against fixed JWKS and OAuth endpoints", async () => {
	const { provider, calls } = await providerFixture();
	const result = await provider.exchange({ code: "code", verifier: "verifier", redirectUri: "http://127.0.0.1:1234/auth/callback", clientId: "oaiapp_fixture", nonce: "nonce-1" });
	assert.equal(result.identity.subject, "subject-1");
	assert.equal(result.identity.email, "fixture@example.com");
	assert.ok(result.credentials.earliestRefreshAt <= Date.now());
	assert.deepEqual(calls, ["https://auth.openai.com/api/accounts/oauth/token", "https://auth.openai.com/.well-known/jwks.json"]);
});

test("provider accepts multiple audiences when azp names the issued client", async () => {
	const { provider } = await providerFixture({ audience: ["oaiapp_fixture", "other-client"], azp: "oaiapp_fixture" });
	const result = await provider.exchange({ code: "code", verifier: "verifier", redirectUri: "http://127.0.0.1:1234/auth/callback", clientId: "oaiapp_fixture", nonce: "nonce-1" });
	assert.equal(result.identity.subject, "subject-1");
});

test("refresh preserves rotated tokens when a returned ID token fails subject validation", async () => {
	const { provider } = await providerFixture({ subject: "different-subject" });
	const result = await provider.refresh({ client_id: "oaiapp_fixture", subject: "subject-1", credentials: { accessToken: "old", refreshToken: "old-refresh", idToken: "old-id", nonce: "nonce-1", scopes: ["resource.invoke", "chatgpt.tokens.use.direct"] } });
	assert.equal(result.credentials.refreshToken, "refresh");
	assert.equal(result.credentials.idToken, "old-id");
	assert.equal(result.validationError.code, "InvalidIdToken");
});

test("invalid refreshed identity remains quarantined across repeated lookup and restart while logout revokes the rotated token", async (t) => {
	const directory = await mkdtemp(join(tmpdir(), "daiw-chat-quarantine-"));
	const { provider, revokedTokens } = await providerFixture({ subject: "different-subject" });
	const projectService = { store: { get() { throw new Error("unused"); } } };
	let chat = new ChatService({ dataDirectory: directory, provider, projectService });
	t.after(async () => { await chat.close(); await rm(directory, { recursive: true, force: true }); });
	const { owner } = chat.createOwner();
	const accountId = chat.store.saveAccount(owner, { clientId: "oaiapp_fixture", subject: "subject-1", profile: { label: "Fixture" }, credentials: { accessToken: "old", refreshToken: "old-refresh", idToken: "old-id", nonce: "nonce-1", expiresAt: Date.now() - 1, scopes: ["resource.invoke", "chatgpt.tokens.use.direct"] } });

	await assert.rejects(chat.auth.credentials(owner, accountId), { code: "InvalidIdToken" });
	await assert.rejects(chat.auth.credentials(owner, accountId), { code: "AccountDisconnected" });
	assert.equal(chat.status(owner, "auto").accounts[0].connected, false);
	await chat.close();
	chat = new ChatService({ dataDirectory: directory, provider, projectService });
	await assert.rejects(chat.auth.credentials(owner, accountId), { code: "AccountDisconnected" });
	assert.equal(chat.status(owner, "auto").activeAccountId, null);
	await chat.auth.logout(owner, accountId);
	assert.deepEqual(revokedTokens, ["refresh"]);
});

test("refresh quarantines an explicitly downgraded scope set", async (t) => {
	const directory = await mkdtemp(join(tmpdir(), "daiw-chat-scope-quarantine-"));
	const { provider } = await providerFixture({}, { scope: "openid resource.invoke" });
	const chat = new ChatService({ dataDirectory: directory, provider, projectService: { store: { get() { throw new Error("unused"); } } } });
	t.after(async () => { await chat.close(); await rm(directory, { recursive: true, force: true }); });
	const { owner } = chat.createOwner();
	const accountId = chat.store.saveAccount(owner, { clientId: "oaiapp_fixture", subject: "subject-1", profile: { label: "Fixture" }, credentials: { accessToken: "old", refreshToken: "old-refresh", idToken: "old-id", nonce: "nonce-1", expiresAt: Date.now() - 1, scopes: ["resource.invoke", "chatgpt.tokens.use.direct"] } });

	await assert.rejects(chat.auth.credentials(owner, accountId), { code: "MissingScope" });
	await assert.rejects(chat.auth.credentials(owner, accountId), { code: "AccountDisconnected" });
	assert.equal(chat.store.account(owner, accountId).credentials.refreshToken, "refresh");
});

test("refresh retains previously granted scopes when the token response omits scope", async () => {
	const { provider } = await providerFixture({}, { scope: undefined });
	const result = await provider.refresh({ client_id: "oaiapp_fixture", subject: "subject-1", credentials: { accessToken: "old", refreshToken: "old-refresh", idToken: "old-id", nonce: "nonce-1", scopes: ["resource.invoke", "chatgpt.tokens.use.direct"] } });
	assert.equal(result.validationError, undefined);
	assert.deepEqual(result.credentials.scopes, ["resource.invoke", "chatgpt.tokens.use.direct"]);
});

test("provider rejects malformed token and model catalog responses with sanitized errors", async () => {
	const fetchImpl = async (input) => String(input).endsWith("/models")
		? Response.json({ models: [{ slug: "ok", display_name: 42, visibility: "list" }] })
		: Response.json({ access_token: "access", refresh_token: "refresh", expires_in: -1, token_type: "mac", scope: "openid" });
	const provider = new OpenAIProvider({ fetchImpl });
	await assert.rejects(provider.exchange({ code: "code", verifier: "verifier", redirectUri: "http://127.0.0.1:1/auth/callback", clientId: "client", nonce: "nonce" }), { code: "InvalidTokenResponse" });
	await assert.rejects(provider.models("secret"), { code: "InvalidModelCatalog" });
});

test("Responses transport ignores hostile OpenAI environment and pins bearer requests", async () => {
	const previous = process.env.OPENAI_CUSTOM_HEADERS;
	process.env.OPENAI_CUSTOM_HEADERS = "X-Hostile: injected\nAuthorization: Bearer attacker";
	const calls = [];
	try {
		const provider = new OpenAIProvider({ fetchImpl: async (input, init) => {
			calls.push({ url: String(input), init, headers: new Headers(init.headers) });
			return new Response('data: {"type":"response.completed","response":{"id":"r","output":[]}}\n\ndata: [DONE]\n\n', { headers: { "content-type": "text/event-stream" } });
		} });
		const stream = await provider.responses({ model: "gpt-test", input: [], store: false, stream: true }, "selected-token", new AbortController().signal);
		for await (const _event of stream) { /* consume */ }
		assert.equal(calls[0].url, "https://api.openai.com/v1/responses");
		assert.equal(calls[0].headers.get("authorization"), "Bearer selected-token");
		assert.equal(calls[0].headers.has("x-hostile"), false);
		assert.equal(calls[0].init.redirect, "error");
	} finally {
		if (previous === undefined) delete process.env.OPENAI_CUSTOM_HEADERS;
		else process.env.OPENAI_CUSTOM_HEADERS = previous;
	}
});

test("Responses transport normalizes upstream errors without exposing provider details", async () => {
	const provider = new OpenAIProvider({ fetchImpl: async () => Response.json({ error: { message: "sensitive upstream detail", type: "invalid_request_error" } }, { status: 400, headers: { "x-request-id": "request-secret" } }) });
	await assert.rejects(
		provider.responses({ model: "gpt-test", input: [], store: false, stream: true }, "selected-token", new AbortController().signal),
		{ code: "ProviderHTTP400", message: "OpenAI Responses request failed with HTTP 400.", status: 400 },
	);
});

for (const [label, overrides, expected] of [
	["wrong audience", { audience: "other-client" }, "InvalidIdToken"],
	["wrong nonce", { nonce: "other-nonce" }, "InvalidIdToken"],
	["expired token", { expiration: "past" }, "InvalidIdToken"],
	["missing expiration", { expiration: undefined }, "InvalidIdToken"],
	["missing issued-at", { issuedAt: false }, "InvalidIdToken"],
	["empty subject", { subject: "" }, "InvalidIdToken"],
	["missing authorized party for multiple audiences", { audience: ["oaiapp_fixture", "other-client"] }, "InvalidIdToken"],
	["mismatched authorized party for multiple audiences", { audience: ["oaiapp_fixture", "other-client"], azp: "other-client" }, "InvalidIdToken"],
	["mismatched authorized party for one audience", { azp: "other-client" }, "InvalidIdToken"],
	["ES256 token", { algorithm: "ES256" }, "InvalidIdToken"],
]) test(`provider rejects ${label}`, async () => {
	const { provider } = await providerFixture(overrides);
	await assert.rejects(provider.exchange({ code: "code", verifier: "verifier", redirectUri: "http://127.0.0.1:1234/auth/callback", clientId: "oaiapp_fixture", nonce: "nonce-1" }), { code: expected });
});
