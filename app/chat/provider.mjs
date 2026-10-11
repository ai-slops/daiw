import { createLocalJWKSet, decodeProtectedHeader, jwtVerify } from "jose";
import OpenAI from "openai";
import { z } from "zod";
import { ServiceError } from "../errors.mjs";

const AUTHORIZE = "https://auth.openai.com/api/accounts/authorize";
const TOKEN = "https://auth.openai.com/api/accounts/oauth/token";
const DISCOVERY = "https://auth.openai.com/.well-known/openid-configuration";
const JWKS = "https://auth.openai.com/.well-known/jwks.json";
const API = "https://api.openai.com/v1";
const requiredScopes = ["resource.invoke", "chatgpt.tokens.use.direct"];
const tokenBase = z.strictObject({
	access_token: z.string().min(1), refresh_token: z.string().min(1),
	token_type: z.string().refine((value) => value.toLowerCase() === "bearer"),
	expires_in: z.number().int().positive(), earliest_refresh_at: z.number().int().nonnegative().optional(),
});
const exchangeToken = tokenBase.extend({ id_token: z.string().min(1), scope: z.string().min(1) });
const refreshToken = tokenBase.extend({ id_token: z.string().min(1).optional(), scope: z.string().min(1).optional() });
const catalogSchema = z.object({ models: z.array(z.object({ slug: z.string().min(1), display_name: z.string().min(1), visibility: z.string().min(1) }).loose()) }).loose();

const safeCode = (value, fallback) => typeof value === "string" && /^[a-z0-9_]{1,80}$/i.test(value) ? value : fallback;
const timeoutSignal = (signal, milliseconds) => signal ? AbortSignal.any([signal, AbortSignal.timeout(milliseconds)]) : AbortSignal.timeout(milliseconds);

export class OpenAIProvider {
	constructor({ fetchImpl = globalThis.fetch } = {}) { this.fetch = fetchImpl; }
	authorizationUrl(parameters) { return `${AUTHORIZE}?${new URLSearchParams(parameters)}`; }
	async #json(url, init, code, schema) {
		let response;
		try { response = await this.fetch(url, { ...init, redirect: "error", signal: timeoutSignal(init?.signal, 15_000) }); }
		catch { throw new ServiceError(code, "OpenAI could not be reached.", 502); }
		if (!response.ok) {
			let upstream;
			try { upstream = await response.json(); } catch { upstream = {}; }
			throw new ServiceError(safeCode(upstream.error, code), `OpenAI rejected the request with HTTP ${response.status}.`, response.status >= 500 ? 502 : 400);
		}
		try { return schema.parse(await response.json()); }
		catch { throw new ServiceError(code, "OpenAI returned an invalid response.", 502); }
	}
	async #token(parameters, schema) {
		return this.#json(TOKEN, { method: "POST", headers: { "content-type": "application/x-www-form-urlencoded" }, body: new URLSearchParams(parameters) }, "InvalidTokenResponse", schema);
	}
	async #identity(idToken, clientId, nonce, subject) {
		try {
			const header = decodeProtectedHeader(idToken);
			if (header.alg !== "RS256") throw new Error("algorithm");
			const jwks = await this.#json(JWKS, {}, "JwksUnavailable", z.strictObject({ keys: z.array(z.record(z.string(), z.unknown())).min(1) }));
			const { payload } = await jwtVerify(idToken, createLocalJWKSet(jwks), { issuer: "https://auth.openai.com", audience: clientId, algorithms: ["RS256"], requiredClaims: ["exp", "iat", "sub"] });
			const audiences = Array.isArray(payload.aud) ? payload.aud : [payload.aud];
			if (typeof payload.sub !== "string" || payload.sub.length === 0 || typeof payload.iat !== "number" || (audiences.length > 1 && payload.azp !== clientId) || (payload.azp !== undefined && payload.azp !== clientId) || (nonce !== undefined && payload.nonce !== nonce) || (subject !== undefined && payload.sub !== subject)) throw new Error("claims");
			return { subject: payload.sub, ...(typeof payload.email === "string" ? { email: payload.email } : {}) };
		} catch (error) {
			if (error instanceof ServiceError && error.code === "JwksUnavailable") throw error;
			throw new ServiceError("InvalidIdToken", "The OpenAI identity token was invalid.", 400);
		}
	}
	#credentials(token, prior = {}) {
		return {
			accessToken: token.access_token, refreshToken: token.refresh_token,
			idToken: prior.idToken, expiresAt: Date.now() + token.expires_in * 1000,
			earliestRefreshAt: token.earliest_refresh_at === undefined ? Date.now() : token.earliest_refresh_at * 1000,
			scopes: token.scope === undefined ? prior.scopes : token.scope.split(" ").filter(Boolean),
			nonce: prior.nonce,
		};
	}
	async exchange({ code, verifier, redirectUri, clientId, nonce }) {
		const token = await this.#token({ grant_type: "authorization_code", client_id: clientId, code, code_verifier: verifier, redirect_uri: redirectUri, resource: API }, exchangeToken);
		const identity = await this.#identity(token.id_token, clientId, nonce);
		const credentials = this.#credentials(token, { idToken: token.id_token, nonce, scopes: token.scope.split(" ").filter(Boolean) });
		if (!requiredScopes.every((scope) => credentials.scopes.includes(scope))) throw new ServiceError("MissingScope", "ChatGPT plan permission was not granted.", 403);
		return { identity, credentials };
	}
	async refresh(account) {
		const token = await this.#token({ grant_type: "refresh_token", client_id: account.client_id, refresh_token: account.credentials.refreshToken, resource: API }, refreshToken);
		const credentials = this.#credentials(token, account.credentials);
		if (!requiredScopes.every((scope) => credentials.scopes.includes(scope))) return { credentials, validationError: new ServiceError("MissingScope", "ChatGPT plan permission was not granted.", 403) };
		if (!token.id_token) return { credentials };
		try {
			await this.#identity(token.id_token, account.client_id, account.credentials.nonce, account.subject);
			return { credentials: { ...credentials, idToken: token.id_token } };
		} catch (validationError) { return { credentials, validationError }; }
	}
	async revoke(account) {
		const discovery = await this.#json(DISCOVERY, {}, "DiscoveryUnavailable", z.object({ issuer: z.literal("https://auth.openai.com"), revocation_endpoint: z.url() }).loose());
		if (new URL(discovery.revocation_endpoint).origin !== "https://auth.openai.com") throw new ServiceError("InvalidDiscovery", "OpenAI discovery metadata was invalid.", 502);
		try {
			const response = await this.fetch(discovery.revocation_endpoint, { method: "POST", headers: { "content-type": "application/x-www-form-urlencoded" }, body: new URLSearchParams({ token: account.credentials.refreshToken, token_type_hint: "refresh_token", client_id: account.client_id }), redirect: "error", signal: AbortSignal.timeout(15_000) });
			return response.status === 200;
		} catch { return false; }
	}
	async models(accessToken) {
		return (await this.#json(`${API}/models`, { headers: { authorization: `Bearer ${accessToken}` } }, "InvalidModelCatalog", catalogSchema)).models;
	}
	async responses(request, accessToken, signal) {
		const safeFetch = async (input, init = {}) => {
			const url = new URL(input instanceof Request ? input.url : String(input));
			if (url.href !== `${API}/responses`) throw new ServiceError("ProviderBoundary", "The provider request target was rejected.", 500);
			const source = input instanceof Request ? input : undefined;
			const sourceHeaders = new Headers(source?.headers ?? init.headers);
			const headers = new Headers({ authorization: `Bearer ${accessToken}`, accept: sourceHeaders.get("accept") ?? "application/json", "content-type": sourceHeaders.get("content-type") ?? "application/json" });
			return this.fetch(url, { method: source?.method ?? init.method, body: source?.body ?? init.body, headers, redirect: "error", signal: timeoutSignal(init.signal ?? signal, 120_000), ...(source?.body ? { duplex: "half" } : {}) });
		};
		const client = new OpenAI({ apiKey: accessToken, baseURL: API, organization: null, project: null, defaultHeaders: {}, maxRetries: 0, timeout: 120_000, fetch: safeFetch, logLevel: "off" });
		try { return await client.responses.create(request, { signal }); }
		catch (error) {
			if (!(error instanceof OpenAI.APIError)) throw error;
			const status = Number.isInteger(error.status) ? error.status : 502;
			throw new ServiceError(`ProviderHTTP${status}`, `OpenAI Responses request failed with HTTP ${status}.`, status >= 500 ? 502 : status);
		}
	}
}
