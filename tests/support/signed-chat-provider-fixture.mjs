import { exportJWK, generateKeyPair, SignJWT } from "jose";
import { OpenAIProvider } from "../../app/chat/provider.mjs";

const encoder = new TextEncoder();

const streamResponse = (events, signal, hold = false) => {
	const body = new ReadableStream({
		start(controller) {
			for (const event of events)
				controller.enqueue(
					encoder.encode(`data: ${JSON.stringify(event)}\n\n`),
				);
			if (!hold) {
				controller.enqueue(encoder.encode("data: [DONE]\n\n"));
				controller.close();
				return;
			}
			signal?.addEventListener("abort", () => controller.error(signal.reason), {
				once: true,
			});
		},
	});
	return new Response(body, {
		headers: { "content-type": "text/event-stream" },
	});
};

const completedMessage = (text) => ({
	type: "response.completed",
	response: {
		id: crypto.randomUUID(),
		object: "response",
		output: [
			{
				type: "message",
				role: "assistant",
				content: [{ type: "output_text", text }],
			},
		],
	},
});

const toolCall = (name, argumentsValue) => ({
	type: "function_call",
	id: `qa-function-${name}`,
	call_id: `qa-call-${name}-${crypto.randomUUID()}`,
	namespace: "DAIW",
	name,
	arguments: JSON.stringify(argumentsValue),
});

class SignedProviderFixture {
	#attempts = new Map();
	#privateKey;
	#publicJwk;

	constructor(keys) {
		this.#privateKey = keys.privateKey;
		this.#publicJwk = keys.publicJwk;
		this.provider = new OpenAIProvider({ fetchImpl: this.fetch.bind(this) });
		this.requests = [];
	}

	prepare(code, nonce, identity) {
		this.#attempts.set(code, { nonce, identity });
	}

	async fetch(input, init = {}) {
		const url = String(input);
		if (url.endsWith("/v1/responses"))
			this.requests.push(JSON.parse(String(init.body)));
		if (url.endsWith("/.well-known/jwks.json"))
			return Response.json({ keys: [this.#publicJwk] });
		if (url.endsWith("/.well-known/openid-configuration"))
			return Response.json({
				issuer: "https://auth.openai.com",
				revocation_endpoint: "https://auth.openai.com/oauth/revoke",
			});
		if (url.endsWith("/oauth/revoke"))
			return new Response(null, { status: 200 });
		if (url.endsWith("/api/accounts/oauth/token"))
			return this.#tokenResponse(new URLSearchParams(init.body));
		if (url.endsWith("/v1/models"))
			return Response.json({
				models: [
					{ slug: "gpt-qa", display_name: "GPT QA", visibility: "list" },
				],
			});
		if (url.endsWith("/v1/responses")) return this.#responses(init);
		throw new TypeError(`Unexpected provider URL: ${url}`);
	}

	async #tokenResponse(parameters) {
		if (parameters.get("grant_type") === "refresh_token")
			return Response.json({
				access_token: "qa-refreshed-access",
				refresh_token: "qa-refreshed-refresh",
				token_type: "Bearer",
				expires_in: 3600,
				scope: "openid resource.invoke chatgpt.tokens.use.direct",
			});
		const attempt = this.#attempts.get(parameters.get("code"));
		if (attempt === undefined)
			return Response.json({ error: "invalid_grant" }, { status: 400 });
		const now = Math.floor(Date.now() / 1000);
		const token = await new SignJWT({
			nonce: attempt.nonce,
			email: attempt.identity.email,
		})
			.setProtectedHeader({ alg: "RS256", kid: "qa-key" })
			.setIssuer("https://auth.openai.com")
			.setAudience(parameters.get("client_id"))
			.setSubject(attempt.identity.subject)
			.setIssuedAt(now)
			.setExpirationTime(now + 300)
			.sign(this.#privateKey);
		return Response.json({
			access_token: `qa-access-${attempt.identity.subject}`,
			refresh_token: `qa-refresh-${attempt.identity.subject}`,
			token_type: "Bearer",
			id_token: token,
			expires_in: 3600,
			scope: "openid resource.invoke chatgpt.tokens.use.direct",
		});
	}

	#responses(init) {
		const request = JSON.parse(String(init.body));
		const serialized = JSON.stringify(request.input);
		if (serialized.includes("Trigger safe terminal failure"))
			return streamResponse(
				[
					{
						type: "response.failed",
						response: { error: { code: "usage_limit" } },
					},
				],
				init.signal,
			);
		if (serialized.includes("Wait for cancellation"))
			return streamResponse(
				[{ type: "response.output_text.delta", delta: "Working…" }],
				init.signal,
				true,
			);
		if (serialized.includes("function_call_output")) {
			const text = "All project tools completed for human review.";
			return streamResponse(
				[
					{ type: "response.output_text.delta", delta: text },
					completedMessage(text),
				],
				init.signal,
			);
		}
		if (serialized.includes("every tool contract")) {
			const calls = [
				toolCall("project_context", {}),
				toolCall("project_query_notes", {
					instanceId: this.instanceId,
					scope: "instance",
					selector: { type: "all" },
					maxResults: 16,
				}),
				toolCall("project_explain", { instanceId: this.instanceId }),
				toolCall("change_dry_run", {
					command: { type: "setTempo", tempo: 121 },
				}),
				toolCall("proposal_create", {
					instanceId: this.instanceId,
					request: "Raise the selected phrase for review",
					operations: [{ type: "transpose", semitones: 12 }],
				}),
			];
			return streamResponse(
				[
					...calls.map((item) => ({ type: "response.output_item.done", item })),
					{
						type: "response.completed",
						response: {
							id: crypto.randomUUID(),
							object: "response",
							output: calls,
						},
					},
				],
				init.signal,
			);
		}
		const text = "Project context is ready.";
		return streamResponse(
			[
				{ type: "response.output_text.delta", delta: text },
				completedMessage(text),
			],
			init.signal,
		);
	}
}

export const createSignedProviderFixture = async () => {
	const { privateKey, publicKey } = await generateKeyPair("RS256");
	const publicJwk = {
		...(await exportJWK(publicKey)),
		kid: "qa-key",
		alg: "RS256",
		use: "sig",
	};
	return new SignedProviderFixture({ privateKey, publicJwk });
};
