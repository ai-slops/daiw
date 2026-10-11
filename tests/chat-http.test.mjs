import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { createHandler } from "../app/http.mjs";
import { ChatService } from "../app/chat/service.mjs";

class FakeProvider {
	constructor() {
		this.refreshes = 0;
		this.revocations = 0;
		this.requests = [];
	}
	authorizationUrl(parameters) {
		return `https://auth.openai.com/api/accounts/authorize?${new URLSearchParams(parameters)}`;
	}
	async exchange({ nonce }) {
		return {
			identity: { subject: "subject-1", email: "user@example.com" },
			credentials: {
				accessToken: "access-1", refreshToken: "refresh-1", idToken: "id-token",
				expiresAt: Date.now() + 60_000,
				scopes: ["openid", "resource.invoke", "chatgpt.tokens.use.direct"], nonce,
			},
		};
	}
	async refresh() {
		this.refreshes += 1;
		return { credentials: { accessToken: "access-2", refreshToken: "refresh-2", expiresAt: Date.now() + 60_000 } };
	}
	async revoke() { this.revocations += 1; return true; }
	async models() { return [{ slug: "gpt-test", display_name: "GPT Test", visibility: "list" }, { slug: "hidden", display_name: "Hidden", visibility: "hide" }]; }
	async *responses(request) {
		this.requests.push(request);
		yield { type: "response.output_text.delta", delta: "Hello" };
		yield { type: "response.completed", response: { output: [{ type: "message", role: "assistant", content: [{ type: "output_text", text: "Hello" }] }] } };
	}
}

class SlowProvider extends FakeProvider {
	async *responses(_request, _token, signal) {
		await new Promise((resolve, reject) => {
			const timer = setTimeout(resolve, 60_000);
			signal.addEventListener("abort", () => { clearTimeout(timer); reject(signal.reason); }, { once: true });
		});
	}
}

class RotatingProvider extends FakeProvider {
	constructor() {
		super();
		this.refreshStarted = new Promise((resolve) => { this.markRefreshStarted = resolve; });
		this.refreshReleased = new Promise((resolve) => { this.releaseRefresh = resolve; });
	}
	async refresh() {
		this.markRefreshStarted();
		await this.refreshReleased;
		return { credentials: { accessToken: "access-2", refreshToken: "refresh-2", expiresAt: Date.now() + 60_000 } };
	}
	async revoke(account) { this.revokedToken = account.credentials.refreshToken; return true; }
}

class InvalidGrantProvider extends FakeProvider {
	constructor() { super(); this.failExchange = true; }
	async exchange(options) {
		if (this.failExchange) { this.failExchange = false; throw Object.assign(new Error("rejected"), { code: "invalid_grant", status: 400 }); }
		return super.exchange(options);
	}
}

async function fixture(t, provider = new FakeProvider(), allowedHosts = new Set()) {
	const root = await mkdtemp(join(tmpdir(), "daiw-chat-http-"));
	const staticRoot = join(root, "static");
	await writeFile(staticRoot, "", { flag: "a" }).catch(() => undefined);
	const project = { id: "project-1", revision: "revision-1", instances: [{ id: "instance-1" }] };
	const service = {
		store: { list: () => [{ id: project.id }], get: () => project },
		execute: async () => ({ ok: true, state: project, result: {} }),
	};
	const chat = new ChatService({ dataDirectory: join(root, "chat"), provider, projectService: service });
	const server = createServer(createHandler({ service, media: {}, staticRoot: root, chat, allowedHosts }));
	server.listen(0, "127.0.0.1");
	await once(server, "listening");
	const address = server.address();
	assert.ok(address && typeof address === "object");
	const base = `http://127.0.0.1:${address.port}`;
	const origin = base;
	t.after(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(resolve));
		await chat.close();
		await rm(root, { recursive: true, force: true });
	});
	let cookie;
	const request = async (path, options = {}) => {
		const headers = new Headers(options.headers);
		if (cookie) headers.set("cookie", cookie);
		if (options.method === "POST") headers.set("origin", origin);
		if (options.body) headers.set("content-type", "application/json");
		const response = await fetch(`${base}${path}`, { ...options, headers });
		const setCookie = response.headers.get("set-cookie");
		if (setCookie) cookie = setCookie.split(";", 1)[0];
		return response;
	};
	return { base, origin, provider, request, chat, get cookie() { return cookie; } };
}

async function connect(context) {
	await context.request("/api/chat/auth/status");
	const login = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) })).json();
	const authorize = new URL(login.authorizationUrl);
	const callback = new URL(login.redirectUri);
	callback.searchParams.set("code", "code");
	callback.searchParams.set("state", authorize.searchParams.get("state"));
	callback.searchParams.set("client_id", "oaiapp_fixture");
	await context.request("/api/chat/auth/login/complete", { method: "POST", body: JSON.stringify({ loginId: login.loginId, callbackUrl: callback.href }) });
	return (await (await context.request("/api/chat/auth/status")).json()).activeAccountId;
}

test("chat auth persists an owner-scoped account without exposing credentials", async (t) => {
	const context = await fixture(t);
	const statusResponse = await context.request("/api/chat/auth/status");
	assert.equal(statusResponse.status, 200);
	assert.match(context.cookie, /^daiw_chat_sid=/);
	const initial = await statusResponse.json();
	assert.match(initial.hostId, /^urn:uuid:/);
	assert.deepEqual(initial.accounts, []);

	const loginResponse = await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) });
	const login = await loginResponse.json();
	assert.equal(login.completionMode, "auto");
	const authorize = new URL(login.authorizationUrl);
	assert.equal(authorize.searchParams.get("scope"), "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct");
	const callback = new URL(login.redirectUri);
	callback.searchParams.set("code", "code-1");
	callback.searchParams.set("state", authorize.searchParams.get("state"));
	callback.searchParams.set("client_id", "oaiapp_fixture");
	const completed = await context.request("/api/chat/auth/login/complete", { method: "POST", body: JSON.stringify({ loginId: login.loginId, callbackUrl: callback.href }) });
	assert.equal(completed.status, 200, await completed.text());
	const status = await (await context.request("/api/chat/auth/status")).json();
	assert.equal(status.accounts[0].profile.email, "user@example.com");
	assert.equal(status.activeAccountId, status.accounts[0].id);
	assert.equal(JSON.stringify(status).includes("access-1"), false);
	const polled = await (await context.request(`/api/chat/auth/login/${login.loginId}`)).json();
	assert.deepEqual(polled, { status: "completed", activeAccountId: status.activeAccountId });
});

test("remote HTTPS status selects paste mode and sets a Secure owner cookie", async (t) => {
	const context = await fixture(t, new FakeProvider(), new Set(["preview.example"]));
	const response = await fetch(`${context.base}/api/chat/auth/status`, { headers: { "x-forwarded-host": "preview.example", "x-forwarded-proto": "https" } });
	assert.equal((await response.json()).callbackMode, "paste");
	assert.match(response.headers.get("set-cookie"), /; Secure;/);
	const cookie = response.headers.get("set-cookie").split(";", 1)[0];
	const rejected = await fetch(`${context.base}/api/chat/auth/login`, { method: "POST", headers: { cookie, origin: "https://preview.example", "x-forwarded-host": "preview.example", "x-forwarded-proto": "https", "content-type": "application/json" }, body: JSON.stringify({ mode: "auto" }) });
	assert.equal(rejected.status, 400);
});

test("chat POST requires an exact origin and owner cookie", async (t) => {
	const context = await fixture(t);
	await context.request("/api/chat/auth/status");
	const missingOrigin = await fetch(`${context.base}/api/chat/auth/login`, { method: "POST", headers: { cookie: context.cookie }, body: "{}" });
	assert.equal(missingOrigin.status, 403);
	assert.equal((await missingOrigin.json()).error.code, "PermissionDenied");
	const noCookie = await fetch(`${context.base}/api/chat/auth/login`, { method: "POST", headers: { origin: context.origin }, body: "{}" });
	assert.equal(noCookie.status, 401);
});

test("sessions stream persisted events and replay them after completion", async (t) => {
	const context = await fixture(t);
	const accountId = await connect(context);
	const models = await (await context.request(`/api/chat/models?accountId=${accountId}`)).json();
	assert.deepEqual(models, { models: [{ id: "gpt-test", displayName: "GPT Test" }] });
	const created = await (await context.request("/api/chat/sessions", { method: "POST", body: JSON.stringify({ projectId: "project-1", accountId, model: "gpt-test" }) })).json();
	const turnResponse = await context.request(`/api/chat/sessions/${created.session.id}/turns`, { method: "POST", body: JSON.stringify({ clientTurnId: crypto.randomUUID(), text: "Say hello" }) });
	assert.equal(turnResponse.status, 202);
	const turn = await turnResponse.json();
	await new Promise((resolve) => setTimeout(resolve, 20));
	const events = await (await context.request(`${turn.eventsUrl}?after=0`)).text();
	assert.match(events, /event: text\.delta/);
	assert.match(events, /event: turn\.completed/);
	assert.equal(context.provider.requests[0].store, false);
	assert.equal(context.provider.requests[0].stream, true);
	assert.equal("previous_response_id" in context.provider.requests[0], false);
	const loaded = await (await context.request(`/api/chat/sessions/${created.session.id}`)).json();
	assert.equal(loaded.turns[0].assistantText, "Hello");
	assert.equal(loaded.turns[0].status, "completed");
	assert.equal(loaded.turns[0].lastEventId, 2);
	assert.equal(await (await context.request(`${turn.eventsUrl}?after=${loaded.turns[0].lastEventId}`)).text(), "");
});

test("login callback validates state before its single-use transaction", async (t) => {
	const context = await fixture(t);
	await context.request("/api/chat/auth/status");
	const login = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) })).json();
	const callback = new URL(login.redirectUri);
	callback.searchParams.set("code", "code"); callback.searchParams.set("state", "wrong"); callback.searchParams.set("client_id", "oaiapp_fixture");
	const wrongState = await context.request("/api/chat/auth/login/complete", { method: "POST", body: JSON.stringify({ loginId: login.loginId, callbackUrl: callback.href }) });
	assert.equal(wrongState.status, 400);
	assert.equal((await wrongState.json()).error.code, "InvalidState");
	const pending = await (await context.request(`/api/chat/auth/login/${login.loginId}`)).json();
	assert.equal(pending.status, "pending");
	const authorize = new URL(login.authorizationUrl);
	callback.searchParams.set("state", authorize.searchParams.get("state"));
	callback.searchParams.append("state", authorize.searchParams.get("state"));
	const duplicate = await context.request("/api/chat/auth/login/complete", { method: "POST", body: JSON.stringify({ loginId: login.loginId, callbackUrl: callback.href }) });
	assert.equal((await duplicate.json()).error.code, "InvalidCallback");

	const expiring = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) })).json();
	const row = context.chat.store.db.prepare("SELECT data FROM pending_logins WHERE id=?").get(expiring.loginId);
	const expiredPending = JSON.parse(row.data); expiredPending.expiresAt = Date.now() - 1;
	context.chat.store.db.prepare("UPDATE pending_logins SET data=? WHERE id=?").run(JSON.stringify(expiredPending), expiring.loginId);
	const expiredUrl = new URL(expiring.redirectUri);
	expiredUrl.searchParams.set("code", "code"); expiredUrl.searchParams.set("state", expiredPending.state); expiredUrl.searchParams.set("client_id", "oaiapp_fixture");
	const expired = await context.request("/api/chat/auth/login/complete", { method: "POST", body: JSON.stringify({ loginId: expiring.loginId, callbackUrl: expiredUrl.href }) });
	assert.equal((await expired.json()).error.code, "LoginExpired");
	assert.equal((await (await context.request(`/api/chat/auth/login/${expiring.loginId}`)).json()).status, "expired");
});

test("returning authorization URLs expose an email hint but never the retained ID token", async (t) => {
	const context = await fixture(t);
	const accountId = await connect(context);
	const login = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ accountId, mode: "auto" }) })).json();
	const authorization = new URL(login.authorizationUrl);
	assert.equal(authorization.searchParams.get("login_hint"), "user@example.com");
	assert.equal(authorization.searchParams.has("id_token_hint"), false);
});

test("automatic callback returns a locked-down page and polling reports its account", async (t) => {
	const context = await fixture(t);
	await context.request("/api/chat/auth/status");
	const login = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) })).json();
	const authorization = new URL(login.authorizationUrl);
	const callback = new URL(login.redirectUri);
	callback.searchParams.set("state", authorization.searchParams.get("state")); callback.searchParams.set("code", "sensitive-code"); callback.searchParams.set("client_id", "oaiapp_fixture");
	const response = await fetch(callback);
	const html = await response.text();
	assert.equal(response.status, 200);
	assert.equal(response.headers.get("referrer-policy"), "no-referrer");
	assert.equal(response.headers.get("cache-control"), "no-store");
	assert.match(response.headers.get("content-security-policy"), /^default-src 'none'/);
	assert.match(html, /history\.replaceState/);
	assert.equal(html.includes("sensitive-code"), false);
	assert.equal((await (await context.request(`/api/chat/auth/login/${login.loginId}`)).json()).status, "completed");
});

test("account refresh is serialized and logout rotates credentials before clearing them", async (t) => {
	const context = await fixture(t);
	const accountId = await connect(context);
	const account = context.chat.store.account(context.chat.owner(context.cookie.split("=", 2)[1]), accountId);
	context.chat.store.updateCredentials(account.owner_hash, accountId, { ...account.credentials, expiresAt: Date.now() - 1 });
	const [first, second] = await Promise.all([context.request(`/api/chat/models?accountId=${accountId}`), context.request(`/api/chat/models?accountId=${accountId}`)]);
	assert.equal(first.status, 200); assert.equal(second.status, 200);
	assert.equal(context.provider.refreshes, 1);
	const logout = await context.request("/api/chat/auth/logout", { method: "POST", body: JSON.stringify({ accountId }) });
	assert.deepEqual(await logout.json(), { revocationConfirmed: true });
	assert.equal(context.provider.revocations, 1);
	const models = await context.request(`/api/chat/models?accountId=${accountId}`);
	assert.equal(models.status, 401);
});

test("logout waits for refresh rotation and revokes the newest token", async (t) => {
	const provider = new RotatingProvider();
	const context = await fixture(t, provider);
	const accountId = await connect(context);
	const owner = context.chat.owner(context.cookie.split("=", 2)[1]);
	const account = context.chat.store.account(owner, accountId);
	context.chat.store.updateCredentials(owner, accountId, { ...account.credentials, expiresAt: Date.now() - 1 });
	const models = context.request(`/api/chat/models?accountId=${accountId}`);
	await provider.refreshStarted;
	const logout = context.request("/api/chat/auth/logout", { method: "POST", body: JSON.stringify({ accountId }) });
	provider.releaseRefresh();
	assert.equal((await models).status, 200);
	assert.equal((await logout).status, 200);
	assert.equal(provider.revokedToken, "refresh-2");
	assert.equal(context.chat.store.account(owner, accountId).connected, false);
});

test("an issued client ID survives invalid_grant without becoming an active account", async (t) => {
	const context = await fixture(t, new InvalidGrantProvider());
	await context.request("/api/chat/auth/status");
	const login = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) })).json();
	const authorization = new URL(login.authorizationUrl);
	const callback = new URL(login.redirectUri);
	callback.searchParams.set("state", authorization.searchParams.get("state")); callback.searchParams.set("code", "bad"); callback.searchParams.set("client_id", "oaiapp_retained");
	const failed = await context.request("/api/chat/auth/login/complete", { method: "POST", body: JSON.stringify({ loginId: login.loginId, callbackUrl: callback.href }) });
	assert.equal(failed.status, 500);
	const status = await (await context.request(`/api/chat/auth/login/${login.loginId}`)).json();
	assert.equal(status.status, "failed");
	assert.equal(status.error.message.includes("rejected"), false);
	assert.deepEqual((await (await context.request("/api/chat/auth/status")).json()).accounts, []);
	const retry = await (await context.request("/api/chat/auth/login", { method: "POST", body: JSON.stringify({ mode: "auto" }) })).json();
	assert.equal(new URL(retry.authorizationUrl).searchParams.get("client_id"), "oaiapp_retained");
});

test("one active turn is enforced and cancellation is replayable over SSE", async (t) => {
	const context = await fixture(t, new SlowProvider());
	const accountId = await connect(context);
	const created = await (await context.request("/api/chat/sessions", { method: "POST", body: JSON.stringify({ projectId: "project-1", accountId, model: "gpt-test" }) })).json();
	const first = await context.request(`/api/chat/sessions/${created.session.id}/turns`, { method: "POST", body: JSON.stringify({ clientTurnId: crypto.randomUUID(), text: "wait" }) });
	const turn = await first.json();
	const busy = await context.request(`/api/chat/sessions/${created.session.id}/turns`, { method: "POST", body: JSON.stringify({ clientTurnId: crypto.randomUUID(), text: "second" }) });
	assert.equal(busy.status, 409);
	await context.request(`/api/chat/sessions/${created.session.id}/turns/${turn.turnId}/cancel`, { method: "POST", body: "{}" });
	const events = await (await context.request(turn.eventsUrl)).text();
	assert.match(events, /event: turn\.cancelled/);
	const replay = await (await context.request(turn.eventsUrl, { headers: { "last-event-id": "0" } })).text();
	assert.match(replay, /event: turn\.cancelled/);
});

test("a different browser owner cannot read another owner's sessions", async (t) => {
	const context = await fixture(t);
	const accountId = await connect(context);
	const created = await (await context.request("/api/chat/sessions", { method: "POST", body: JSON.stringify({ projectId: "project-1", accountId, model: "gpt-test" }) })).json();
	const otherStatus = await fetch(`${context.base}/api/chat/auth/status`);
	const otherCookie = otherStatus.headers.get("set-cookie").split(";", 1)[0];
	const isolated = await fetch(`${context.base}/api/chat/sessions/${created.session.id}`, { headers: { cookie: otherCookie } });
	assert.equal(isolated.status, 404);
	assert.equal((await isolated.json()).error.code, "MissingSession");
});
