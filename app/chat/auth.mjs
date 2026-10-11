import { createHash, randomBytes, randomUUID } from "node:crypto";
import { createServer } from "node:http";
import { ServiceError } from "../errors.mjs";
import { parseCallback, sendCallbackPage } from "./callback.mjs";

const random = () => randomBytes(32).toString("base64url");
const challenge = (verifier) => createHash("sha256").update(verifier).digest("base64url");
const SCOPE = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

export class ChatAuth {
	#listeners = new Map();
	#locks = new Map();
	constructor({ store, provider }) { this.store = store; this.provider = provider; }
	status(owner, callbackMode) {
		return { hostId: this.store.meta("hostId", () => `urn:uuid:${randomUUID()}`), callbackMode, accounts: this.store.accounts(owner).map((account) => ({ id: account.id, connected: account.connected, profile: account.profile })), activeAccountId: this.store.active(owner) };
	}
	loginStatus(owner, loginId) {
		const pending = this.store.pending(loginId, owner);
		if (pending.status === "completed") return { status: "completed", activeAccountId: pending.activeAccountId };
		if (pending.status === "failed") return { status: "failed", error: pending.error };
		if (pending.status === "expired") return { status: "expired" };
		return { status: "pending" };
	}
	async login(owner, input, callbackMode) {
		if (input.mode !== undefined && input.mode !== callbackMode) throw new ServiceError("InvalidCallbackMode", `This origin requires ${callbackMode} callback mode.`, 400);
		const account = input.accountId ? this.store.account(owner, input.accountId) : null;
		const registration = account ? null : this.store.registration(owner);
		const server = createServer();
		server.listen(0, "127.0.0.1");
		await new Promise((resolve, reject) => { server.once("listening", resolve); server.once("error", reject); });
		const address = server.address();
		if (!address || typeof address === "string") throw new ServiceError("CallbackFailed", "Could not create the loopback callback.", 500);
		const loginId = randomUUID();
		const pending = { expiresAt: Date.now() + 600_000, state: random(), nonce: random(), verifier: random(), redirectUri: `http://127.0.0.1:${address.port}/auth/callback`, accountId: account?.id ?? null, requestedClientId: account?.client_id ?? registration?.clientId ?? "dynamic_agent_client" };
		this.store.savePending(loginId, owner, pending);
		const timer = setTimeout(() => { this.store.finishPending(loginId, owner, { status: "expired" }); this.#closeListener(loginId); }, 600_000);
		this.#listeners.set(loginId, { server, timer });
		server.on("request", (request, response) => this.#autoComplete(loginId, owner, request, response));
		const parameters = { client_id: pending.requestedClientId, ext_agent_host_id: this.store.meta("hostId", () => `urn:uuid:${randomUUID()}`), response_type: "code", redirect_uri: pending.redirectUri, scope: SCOPE, resource: "https://api.openai.com/v1", state: pending.state, nonce: pending.nonce, code_challenge_method: "S256", code_challenge: challenge(pending.verifier) };
		if (pending.requestedClientId === "dynamic_agent_client") parameters.agent_name_hint = "DAIW";
		if (account?.profile?.email) parameters.login_hint = account.profile.email;
		return { loginId, authorizationUrl: this.provider.authorizationUrl(parameters), redirectUri: pending.redirectUri, completionMode: callbackMode, expiresAt: pending.expiresAt };
	}
	async #autoComplete(loginId, owner, request, response) {
		const pending = this.store.pendingRaw(loginId, owner);
		if (request.method !== "GET" || request.headers.host !== new URL(pending.redirectUri).host) { await sendCallbackPage(response, false); return; }
		try { await this.complete(owner, loginId, `http://${request.headers.host}${request.url}`, false); await sendCallbackPage(response, true); }
		catch { await sendCallbackPage(response, false); }
		finally { this.#closeListener(loginId); }
	}
	async complete(owner, loginId, callbackUrl, closeListener = true) {
		const pending = this.store.pending(loginId, owner);
		if (pending.status === "expired") { this.#closeListener(loginId); throw new ServiceError("LoginExpired", "This login attempt expired.", 400); }
		if (pending.status !== "pending") throw new ServiceError("InvalidLogin", "Login attempt is no longer pending.", 409);
		if (pending.expiresAt <= Date.now()) { this.store.finishPending(loginId, owner, { status: "expired" }); this.#closeListener(loginId); throw new ServiceError("LoginExpired", "This login attempt expired.", 400); }
		const callback = parseCallback(pending, callbackUrl);
		if (callback.error) { this.store.claimPending(loginId, owner); this.store.finishPending(loginId, owner, { status: "failed", error: callback.error }); if (closeListener) this.#closeListener(loginId); throw new ServiceError(callback.error.code, callback.error.message, 400); }
		if (pending.requestedClientId === "dynamic_agent_client") this.store.saveRegistration(owner, callback.clientId);
		this.store.claimPending(loginId, owner);
		try {
			const result = await this.provider.exchange({ code: callback.code, verifier: pending.verifier, redirectUri: pending.redirectUri, clientId: callback.clientId, nonce: pending.nonce });
			if (pending.accountId) { const account = this.store.account(owner, pending.accountId); if (account.client_id !== callback.clientId || account.subject !== result.identity.subject) throw new ServiceError("AccountMismatch", "The returned account did not match the selected registration.", 403); }
			const activeAccountId = this.store.saveAccount(owner, { clientId: callback.clientId, subject: result.identity.subject, profile: { ...(result.identity.email ? { email: result.identity.email } : {}), label: result.identity.email ?? `ChatGPT ${result.identity.subject.slice(0, 8)}` }, credentials: result.credentials });
			this.store.finishPending(loginId, owner, { status: "completed", activeAccountId });
			return { activeAccountId };
		} catch (error) {
			const normalized = { code: error instanceof ServiceError ? error.code : "AuthorizationFailed", message: error instanceof ServiceError ? error.message : "ChatGPT authorization failed." };
			this.store.finishPending(loginId, owner, { status: "failed", error: normalized });
			throw error;
		} finally { if (closeListener) this.#closeListener(loginId); }
	}
	async credentials(owner, accountId) {
		const current = this.store.account(owner, accountId);
		if (!current.connected || !current.credentials) throw new ServiceError("AccountDisconnected", "This ChatGPT account is signed out.", 401);
		if (current.credentials.expiresAt > Date.now() + 30_000) return current.credentials;
		return this.#locked(accountId, async () => {
			const account = this.store.account(owner, accountId);
			if (!account.connected || !account.credentials) throw new ServiceError("AccountDisconnected", "This ChatGPT account is signed out.", 401);
			if (account.credentials.expiresAt > Date.now() + 30_000) return account.credentials;
				if ((account.credentials.earliestRefreshAt ?? 0) > Date.now() && account.credentials.expiresAt > Date.now()) return account.credentials;
				const refreshed = await this.provider.refresh(account);
				if (refreshed.validationError) { this.store.quarantineCredentials(owner, accountId, refreshed.credentials); throw refreshed.validationError; }
				this.store.updateCredentials(owner, accountId, refreshed.credentials);
			return refreshed.credentials;
		});
	}
	logout(owner, accountId) {
		return this.#locked(accountId, async () => {
			const account = this.store.account(owner, accountId);
			let revocationConfirmed = false;
			try { if (account.credentials) revocationConfirmed = await this.provider.revoke(account); }
			finally { this.store.disconnect(owner, accountId); }
			return { revocationConfirmed };
		});
	}
	#locked(key, operation) {
		const prior = this.#locks.get(key) ?? Promise.resolve();
		const task = prior.catch(() => undefined).then(operation);
		this.#locks.set(key, task);
		return task.finally(() => { if (this.#locks.get(key) === task) this.#locks.delete(key); });
	}
	#closeListener(id) { const listener = this.#listeners.get(id); if (listener) { clearTimeout(listener.timer); listener.server.closeAllConnections(); listener.server.close(); this.#listeners.delete(id); } }
	close() { for (const id of this.#listeners.keys()) this.#closeListener(id); }
}
