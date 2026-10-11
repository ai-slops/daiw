import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerTools, turnSchema } from "../app/chat/schemas.mjs";
import { ChatStore } from "../app/chat/store.mjs";
import { ChatTools } from "../app/chat/tools.mjs";

async function fixture(t) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-chat-tools-"));
	const store = new ChatStore(directory);
	const project = { id: "project-1", name: "Song", revision: "revision-1", tempo: 120, meter: [4, 4], tracks: [], definitions: [], instances: [] };
	const calls = [];
	const projectService = {
		store: { get: () => { throw new Error("tools must not bypass ProjectService"); } },
		execute: async (request, principal) => { calls.push({ request, principal }); return { ok: true, state: request.method === "proposal.create" ? { ...project, revision: "revision-2" } : project, result: { accepted: true } }; },
	};
	const tools = new ChatTools({ store, projectService });
	t.after(async () => { store.close(); await rm(directory, { recursive: true, force: true }); });
	return { calls, project, store, tools };
}

test("proposal tool injects project identity and captured revision under the agent principal", async (t) => {
	const { calls, tools } = await fixture(t);
	const context = { turnId: "turn-1", projectId: "project-1", revision: "revision-1", signal: new AbortController().signal };
	const call = { callId: "call-1", namespace: "DAIW", name: "proposal_create", arguments: JSON.stringify({ instanceId: "instance-1", request: "make it louder" }) };
	const result = await tools.execute(context, call);
	assert.deepEqual(result, { output: { accepted: true }, revision: "revision-2" });
	assert.equal(calls[0].principal, "agent");
	assert.equal(calls[0].request.projectId, "project-1");
	assert.equal(calls[0].request.params.expectedRevision, "revision-1");
	assert.equal(calls[0].request.expectedRevision, "revision-1");
	assert.equal(calls[0].request.idempotencyKey, "chat:turn-1:call-1");
	assert.deepEqual(await tools.execute(context, call), result);
	assert.equal(calls.length, 1);
});

test("tool schemas reject model-supplied project identity and forbidden methods", async (t) => {
	const { tools } = await fixture(t);
	const context = { turnId: "turn-1", projectId: "project-1", revision: "revision-1", signal: new AbortController().signal };
	await assert.rejects(tools.execute(context, { callId: "call-1", namespace: "DAIW", name: "change_dry_run", arguments: JSON.stringify({ projectId: "other", command: { type: "setTempo", tempo: 90 } }) }));
	await assert.rejects(tools.execute(context, { callId: "call-2", namespace: "DAIW", name: "change.apply", arguments: "{}" }), { code: "ToolDenied" });
});

test("project context is read through the queued agent boundary", async (t) => {
	const { calls, tools } = await fixture(t);
	const context = { turnId: "turn-1", projectId: "project-1", revision: "revision-1", signal: new AbortController().signal };
	const result = await tools.execute(context, { callId: "call-1", name: "project_context", namespace: "DAIW", arguments: "{}" });
	assert.equal(calls[0].request.method, "project.get");
	assert.equal(calls[0].request.expectedRevision, "revision-1");
	assert.equal(result.output.revision, "revision-1");
});

test("provider tool strictness matches each JSON schema closure", () => {
	const tools = providerTools[0].tools;
	const dryRun = tools.find((tool) => tool.name === "change_dry_run");
	assert.equal(dryRun.strict, false);
	for (const tool of tools.filter((candidate) => candidate.strict)) {
		assert.equal(tool.parameters.additionalProperties, false);
		for (const property of Object.values(tool.parameters.properties)) {
			if (property.type === "object") assert.equal(property.additionalProperties, false);
		}
	}
});

test("turn text limit is measured in UTF-8 bytes", () => {
	const identifier = "00000000-0000-4000-8000-000000000000";
	assert.equal(turnSchema.safeParse({ clientTurnId: identifier, text: "a".repeat(16 * 1024) }).success, true);
	assert.equal(turnSchema.safeParse({ clientTurnId: identifier, text: "한".repeat(6 * 1024) }).success, false);
});
