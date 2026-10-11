import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { ChatService } from "../app/chat/service.mjs";

const completed = (output = []) => ({ type: "response.completed", response: { id: crypto.randomUUID(), output } });
const functionCall = (callId, name, argumentsValue = {}) => ({ type: "function_call", call_id: callId, namespace: "DAIW", name, arguments: JSON.stringify(argumentsValue) });

async function fixture(t, provider) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-chat-runtime-"));
	let revision = "revision-1";
	const calls = [];
	const project = () => ({ id: "project-1", name: "Song", revision, tempo: 120, meter: [4, 4], tracks: [], definitions: [], instances: [{ id: "instance-1" }] });
	const projectService = {
		store: { get: () => project() },
		execute: async (request, principal) => {
			calls.push({ request, principal });
			if (request.expectedRevision !== revision) throw Object.assign(new Error("stale"), { code: "StaleRevision", status: 409 });
			if (request.method === "proposal.create") revision = "revision-2";
			return { ok: true, state: project(), result: { method: request.method } };
		},
	};
	const chat = new ChatService({ dataDirectory: directory, provider, projectService });
	const { owner } = chat.createOwner();
	const accountId = chat.store.saveAccount(owner, { clientId: "oaiapp_fixture", subject: "subject-1", profile: { label: "Fixture" }, credentials: { accessToken: "access", refreshToken: "refresh", expiresAt: Date.now() + 60_000, scopes: ["resource.invoke", "chatgpt.tokens.use.direct"] } });
	const session = chat.store.createSession(owner, { projectId: "project-1", accountId, model: "gpt-test" });
	t.after(async () => { await chat.close(); await rm(directory, { recursive: true, force: true }); });
	return { accountId, calls, chat, owner, session };
}

async function waitTerminal(chat, turnId) {
	const current = chat.store.turn(turnId);
	if (current.status !== "running") return current;
	await new Promise((resolve) => {
		const listener = (event) => {
			if (event.name.startsWith("turn.")) { chat.events.off(turnId, listener); resolve(); }
		};
		chat.events.on(turnId, listener);
	});
	return chat.store.turn(turnId);
}

async function waitFor(predicate) {
	for (let attempt = 0; attempt < 100; attempt += 1) {
		if (predicate()) return;
		await new Promise((resolve) => setTimeout(resolve, 10));
	}
	throw new Error("Timed out waiting for condition");
}

test("runner bounds streamed display output before persisting an oversized delta", async (t) => {
	const provider = { async *responses() { yield { type: "response.output_text.delta", delta: "x".repeat(256 * 1024 + 1) }; yield completed(); } };
	const { chat, owner, session } = await fixture(t, provider);
	const { turnId } = chat.startTurn(owner, session.id, { clientTurnId: crypto.randomUUID(), text: "overflow" });
	const turn = await waitTerminal(chat, turnId);
	assert.equal(turn.status, "failed");
	assert.equal(turn.error.code, "OutputLimit");
	assert.equal(turn.assistant_text, "");
});

test("runner reserves a terminal event within the 2048 event limit", async (t) => {
	const provider = { async *responses() { for (let index = 0; index < 2050; index += 1) yield { type: "response.output_text.delta", delta: "x" }; } };
	const { chat, owner, session } = await fixture(t, provider);
	const { turnId } = chat.startTurn(owner, session.id, { clientTurnId: crypto.randomUUID(), text: "flood" });
	const turn = await waitTerminal(chat, turnId);
	const events = chat.store.events(turnId, 0, 4096);
	assert.equal(turn.error.code, "EventLimit");
	assert.equal(events.length, 2048);
	assert.equal(events.at(-1).name, "turn.failed");
});

test("runner handles namespaced SDK function calls and advances only its proposal revision", async (t) => {
	const requests = [];
	let responseIndex = 0;
	const provider = { async *responses(request, _token, signal) {
		requests.push(request);
		responseIndex += 1;
			if (responseIndex === 1) { const call = functionCall("proposal", "proposal_create", { instanceId: "instance-1", request: "louder" }); yield { type: "response.output_item.done", item: call }; yield completed(); return; }
			if (responseIndex === 2) { const call = functionCall("context", "project_context"); yield { type: "response.output_item.done", item: call }; yield completed(); return; }
		await new Promise((_resolve, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true }));
	} };
	const { calls, chat, owner, session } = await fixture(t, provider);
	const { turnId } = chat.startTurn(owner, session.id, { clientTurnId: crypto.randomUUID(), text: "propose" });
	await waitFor(() => chat.store.tool(turnId, "context")?.status === "completed");
	chat.cancel(owner, session.id, turnId);
	const turn = await waitTerminal(chat, turnId);
	assert.equal(turn.status, "cancelled");
	assert.deepEqual(turn.history.map((item) => item.type), ["function_call", "function_call_output", "function_call", "function_call_output"]);
	assert.deepEqual(requests[0].tools[0].tools.map((tool) => tool.name), ["project_context", "project_query_notes", "project_explain", "change_dry_run", "proposal_create"]);
	assert.equal(calls[0].request.expectedRevision, "revision-1");
	assert.equal(calls[1].request.expectedRevision, "revision-2");
	assert.ok(requests[1].input.some((item) => item.type === "function_call_output"));
});
