import { randomBytes, timingSafeEqual } from "node:crypto";
import { ServiceError } from "../errors.mjs";

const safeEqual = (left, right) => {
	if (typeof left !== "string" || typeof right !== "string" || Buffer.byteLength(left) !== Buffer.byteLength(right)) return false;
	return timingSafeEqual(Buffer.from(left), Buffer.from(right));
};

const one = (parameters, name, required = true) => {
	const values = parameters.getAll(name);
	if (values.length > 1 || (required && values.length !== 1) || values[0] === "") throw new ServiceError("InvalidCallback", `The callback ${name} parameter was invalid.`, 400);
	return values[0] ?? null;
};

export function parseCallback(pending, callbackUrl) {
	const callback = new URL(callbackUrl);
	const expected = new URL(pending.redirectUri);
	if (callback.protocol !== "http:" || callback.hostname !== "127.0.0.1" || callback.host !== expected.host || callback.pathname !== "/auth/callback" || callback.username || callback.password || callback.hash) throw new ServiceError("InvalidCallback", "The callback URL does not match this login attempt.", 400);
	const state = one(callback.searchParams, "state");
	if (!safeEqual(state, pending.state)) throw new ServiceError("InvalidState", "The OAuth state did not match.", 400);
	const oauthError = one(callback.searchParams, "error", false);
	const code = one(callback.searchParams, "code", false);
	const suppliedClientId = one(callback.searchParams, "client_id", false);
	if (oauthError) return { error: { code: "AuthorizationDenied", message: "ChatGPT authorization was denied." } };
	if (!code || (pending.requestedClientId === "dynamic_agent_client" && !suppliedClientId)) throw new ServiceError("InvalidCallback", "The callback code or client ID was missing.", 400);
	const clientId = pending.requestedClientId === "dynamic_agent_client" ? suppliedClientId : pending.requestedClientId;
	if (!clientId || clientId === "dynamic_agent_client" || (suppliedClientId && suppliedClientId !== clientId)) throw new ServiceError("InvalidCallback", "The callback client ID did not match.", 400);
	return { code, clientId };
}

export async function sendCallbackPage(response, success) {
	const nonce = randomBytes(16).toString("base64url");
	const safeMessage = success ? "ChatGPT sign-in completed. You can return to DAIW." : "ChatGPT sign-in failed. Return to DAIW and try again.";
	const finished = new Promise((resolve) => response.once("finish", resolve));
	response.writeHead(success ? 200 : 400, {
		"cache-control": "no-store", "content-type": "text/html; charset=utf-8",
		"content-security-policy": `default-src 'none'; script-src 'nonce-${nonce}'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`,
		"referrer-policy": "no-referrer", "x-content-type-options": "nosniff",
	});
	response.end(`<!doctype html><meta charset="utf-8"><title>DAIW sign-in</title><p>${safeMessage}</p><script nonce="${nonce}">history.replaceState(null,"","/auth/callback")</script>`);
	await finished;
}
