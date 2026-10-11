import assert from "node:assert/strict";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { NativeAudioWorker } from "../app/native-audio-worker.mjs";

const state = {
	status: "stopped",
	positionFrame: 0,
	frames: 64,
	loop: null,
	stats: { underrunCallbacks: 0, staleBlocks: 0, deviceErrors: 0 },
};

async function workerFixture(t, source, options = {}) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-native-worker-"));
	const binary = join(directory, "fake-worker");
	await writeFile(binary, `#!/usr/bin/env node\n${source}`);
	await chmod(binary, 0o755);
	const worker = await NativeAudioWorker.start({
		binary,
		args: [],
		onState: () => undefined,
		...options,
	});
	t.after(async () => {
		await worker.close();
		await rm(directory, { recursive: true, force: true });
	});
	return worker;
}

test("worker parses complete lines before enforcing the pending-fragment limit", async (t) => {
	// Given
	const worker = await workerFixture(
		t,
		`const ready = ${JSON.stringify({ v: 1, type: "ready", frames: 64 })};
const state = ${JSON.stringify(state)};
process.stdout.write(JSON.stringify(ready) + "\\n");
process.stdin.on("data", (chunk) => {
  const command = JSON.parse(chunk.toString().trim());
  const events = Array.from({ length: 80 }, (_, sequence) => JSON.stringify({ v: 1, type: "state", sequence, state })).join("\\n");
  process.stdout.write(events + "\\n" + JSON.stringify({ v: 1, type: "response", id: command.id, ok: true, state }) + "\\n");
  if (command.type === "shutdown") process.exit(0);
});`,
	);
	// When
	const result = await worker.command({ type: "state" });
	// Then
	assert.equal(result.frames, 64);
});

test("worker rejects a command that exceeds its bounded response time", async (t) => {
	// Given
	const worker = await workerFixture(
		t,
		`process.stdout.write(${JSON.stringify(`${JSON.stringify({ v: 1, type: "ready", frames: 64 })}\n`)});
process.stdin.resume();`,
		{ commandTimeoutMs: 20 },
	);
	// When / Then
	await assert.rejects(worker.command({ type: "state" }), {
		code: "CommandTimeout",
	});
});
