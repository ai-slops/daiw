import assert from "node:assert/strict";
import { test } from "node:test";
import { OpenAIProvider } from "../app/chat/provider.mjs";
import {
	parseAuthorizationUrl,
	registerAuthHandler,
} from "../desktop/auth-ipc.cjs";

const origin = "http://127.0.0.1:3333";

test("Electron auth accepts only the OpenAI OAuth authorization endpoint", () => {
	const generatedUrl = new OpenAIProvider().authorizationUrl({
		client_id: "daiw",
		redirect_uri: "http://127.0.0.1:4567/auth/callback",
		response_type: "code",
		state: "opaque",
	});
	assert.equal(parseAuthorizationUrl(generatedUrl), generatedUrl);
	for (const value of [
		"http://auth.openai.com/oauth/authorize",
		"https://auth.openai.com/oauth/authorize",
		"https://auth.openai.com/api/accounts/oauth/token",
		"https://auth.openai.com.evil.test/api/accounts/authorize",
		"https://user@auth.openai.com/api/accounts/authorize",
		"https://auth.openai.com:444/api/accounts/authorize",
		"https://auth.openai.com/api/accounts/authorize#fragment",
		"not a URL",
		undefined,
	]) {
		assert.throws(() => parseAuthorizationUrl(value), {
			code: "InvalidAuthorizationUrl",
		});
	}
});

test("Electron auth opens a validated URL only for the owned main frame", async () => {
	const handlers = new Map();
	const opened = [];
	registerAuthHandler({
		ipcMain: {
			handle: (channel, handler) => handlers.set(channel, handler),
		},
		shell: {
			openExternal: async (...args) => opened.push(args),
		},
		origin,
	});
	const handler = handlers.get("daiw-auth:open-authorization");
	const mainFrame = { url: `${origin}/` };
	const event = { sender: { mainFrame }, senderFrame: mainFrame };
	const url = "https://auth.openai.com/api/accounts/authorize?state=opaque";

	assert.deepEqual(await handler(event, { url }), { opened: true });
	assert.deepEqual(opened, [[url, { activate: true }]]);
	await assert.rejects(
		handler({ ...event, senderFrame: { url: `${origin}/embedded` } }, { url }),
		{ code: "UntrustedFrame" },
	);
	assert.equal(opened.length, 1);
});
