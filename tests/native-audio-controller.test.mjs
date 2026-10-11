import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { NativeAudioController } from "../app/native-audio-controller.mjs";

function wav(frames = 64) {
	const bytes = Buffer.alloc(44 + frames * 4);
	bytes.write("RIFF", 0);
	bytes.writeUInt32LE(bytes.length - 8, 4);
	bytes.write("WAVEfmt ", 8);
	bytes.writeUInt32LE(16, 16);
	bytes.writeUInt16LE(1, 20);
	bytes.writeUInt16LE(2, 22);
	bytes.writeUInt32LE(48_000, 24);
	bytes.writeUInt32LE(192_000, 28);
	bytes.writeUInt16LE(4, 32);
	bytes.writeUInt16LE(16, 34);
	bytes.write("data", 36);
	bytes.writeUInt32LE(frames * 4, 40);
	return bytes;
}

async function fixture(t, options = {}) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-native-controller-"));
	const managedPath = join(directory, "render.wav");
	const bytes = wav();
	await writeFile(managedPath, bytes);
	let revision = "revision-1";
	let revisionReads = 0;
	let closes = 0;
	let renders = 0;
	const media = {
		render: async () => {
			renders += 1;
			return {
				handle: {
					close: async () => {
						closes += 1;
					},
				},
				managedPath,
				revision,
				descriptor: {
					sha256: createHash("sha256").update(bytes).digest("hex"),
				},
			};
		},
	};
	const service = {
		store: {
			get: () => ({
				revision:
					options.revisionAtRead?.(++revisionReads, revision) ?? revision,
			}),
		},
	};
	const states = [];
	const controller = new NativeAudioController({
		binary: options.binary ?? resolve("target/debug/daiw-play"),
		media,
		service,
		now: options.now,
		headlessCallbackDriver: options.headlessCallbackDriver ?? true,
		onState: (_clientId, state) => states.push(state),
	});
	t.after(async () => {
		await controller.close();
		await rm(directory, { recursive: true, force: true });
	});
	return {
		controller,
		states,
		setRevision: (value) => {
			revision = value;
		},
		closes: () => closes,
		renders: () => renders,
	};
}

async function fakeWorker(t, source) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-native-fake-worker-"));
	const binary = join(directory, "fake-daiw-play");
	await writeFile(binary, `#!/usr/bin/env node\n${source}`);
	await chmod(binary, 0o755);
	t.after(() => rm(directory, { recursive: true, force: true }));
	return { binary, directory };
}

async function waitFor(predicate, timeoutMs = 1_000) {
	const deadline = performance.now() + timeoutMs;
	while (performance.now() < deadline) {
		if (await predicate()) return true;
		await new Promise((resolvePromise) => setTimeout(resolvePromise, 10));
	}
	return false;
}

test("controller drives the real headless callback and closes the render handle after ready", async (t) => {
	// Given
	const { controller, states, closes } = await fixture(t);
	// When
	const prepared = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	await controller.command("client-1", prepared.sessionToken, { type: "play" });
	await controller.advanceForTest("client-1", prepared.sessionToken, 32);
	const state = await controller.command("client-1", prepared.sessionToken, {
		type: "state",
	});
	// Then
	assert.equal(closes(), 1);
	assert.equal(state.status, "playing");
	assert.equal(state.positionFrame, 32);
	assert.ok(states.some((item) => item.state.positionFrame === 32));
	assert.equal(states.at(-1).sessionToken, prepared.sessionToken);
});

test("controller reports a missing native player without exposing its path", async (t) => {
	// Given
	const binary = join(tmpdir(), `missing-daiw-play-${process.pid}`);
	const { controller } = await fixture(t, { binary });
	// When
	const capabilities = await controller.capabilities();
	// Then
	assert.equal(capabilities.available, false);
	await assert.rejects(controller.listDevices("client-1"), (error) => {
		assert.equal(error.code, "NativeUnavailable");
		assert.equal(error.message.includes(binary), false);
		return true;
	});
});

test("production preparation requires a host-issued device token before rendering", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n");`,
	);
	const { controller, renders } = await fixture(t, {
		binary,
		headlessCallbackDriver: false,
	});
	// When / Then
	await assert.rejects(
		controller.prepare("client-1", {
			projectId: "project-1",
			expectedRevision: "revision-1",
		}),
		{ code: "InvalidDevice" },
	);
	assert.equal(renders(), 0);
});

test("production preparation rejects an unsupported issued device before rendering", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`if (process.argv.includes("--list-output-devices")) {
  process.stdout.write(JSON.stringify({ v: 1, devices: [{ index: 0, name: "Unsupported", nameSha256: "${"b".repeat(64)}", isDefault: true, supported: false }] }));
} else {
  process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n");
}`,
	);
	const { controller, renders } = await fixture(t, {
		binary,
		headlessCallbackDriver: false,
	});
	const listed = await controller.listDevices("client-1");
	// When / Then
	await assert.rejects(
		controller.prepare("client-1", {
			projectId: "project-1",
			expectedRevision: "revision-1",
			deviceToken: listed.devices[0].token,
		}),
		{ code: "InvalidDevice" },
	);
	assert.equal(renders(), 0);
});

test("session release preserves an issued device token for repeated preparation", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`const state = { status: "stopped", positionFrame: 0, frames: 64, loop: null, stats: { underrunCallbacks: 0, staleBlocks: 0, deviceErrors: 0 } };
if (process.argv.includes("--list-output-devices")) {
  process.stdout.write(JSON.stringify({ v: 1, devices: [{ index: 0, name: "Supported", nameSha256: "${"c".repeat(64)}", isDefault: true, supported: true }] }));
} else {
  process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n");
  process.stdin.on("data", (chunk) => {
    for (const line of chunk.toString().trim().split("\\n")) {
      const command = JSON.parse(line);
      process.stdout.write(JSON.stringify({ v: 1, type: "response", id: command.id, ok: true, state }) + "\\n");
      if (command.type === "shutdown") process.exit(0);
    }
  });
}`,
	);
	const { controller } = await fixture(t, {
		binary,
		headlessCallbackDriver: false,
	});
	const listed = await controller.listDevices("client-1");
	const request = {
		projectId: "project-1",
		expectedRevision: "revision-1",
		deviceToken: listed.devices[0].token,
	};
	const first = await controller.prepare("client-1", request);
	// When
	await controller.release("client-1", first.sessionToken);
	const second = await controller.prepare("client-1", request);
	// Then
	assert.notEqual(second.sessionToken, first.sessionToken);
});

test("controller rejects stale preparation and supersedes one session per client", async (t) => {
	// Given
	const { controller, setRevision } = await fixture(t);
	const first = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	// When
	const second = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	setRevision("revision-2");
	// Then
	await assert.rejects(
		controller.command("client-1", first.sessionToken, { type: "state" }),
		{ code: "InvalidSession" },
	);
	await assert.rejects(
		controller.prepare("client-2", {
			projectId: "project-1",
			expectedRevision: "revision-1",
		}),
		{ code: "StaleRevision" },
	);
	await controller.release("client-1", second.sessionToken);
});

test("controller rejects play when the prepared project revision has changed", async (t) => {
	// Given
	const { controller, setRevision } = await fixture(t);
	const prepared = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	setRevision("revision-2");
	// When / Then
	await assert.rejects(
		controller.command("client-1", prepared.sessionToken, { type: "play" }),
		{ code: "StaleRevision" },
	);
});

test("controller keeps output device tokens valid until the list is refreshed", async (t) => {
	// Given
	const directory = await mkdtemp(join(tmpdir(), "daiw-native-device-list-"));
	const binary = join(directory, "fake-daiw-play");
	await writeFile(
		binary,
		`#!/usr/bin/env node
if (process.argv.includes("--list-output-devices")) {
  process.stdout.write(JSON.stringify({ v: 1, devices: [{ index: 0, name: "Test output", nameSha256: "${"a".repeat(64)}", isDefault: true, supported: true }] }));
} else {
  process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n");
  process.stdin.on("data", (chunk) => {
    const command = JSON.parse(chunk.toString().trim());
    if (command.type === "shutdown") process.exit(0);
  });
}
`,
	);
	await chmod(binary, 0o755);
	let now = 1_000;
	const { controller } = await fixture(t, { binary, now: () => now });
	t.after(() => rm(directory, { recursive: true, force: true }));
	const listed = await controller.listDevices("client-1");
	now += 300_001;
	// When
	const prepared = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
		deviceToken: listed.devices[0].token,
	});
	// Then
	assert.equal(typeof prepared.sessionToken, "string");
});

test("controller invalidates output device tokens on refresh and client release", async (t) => {
	// Given
	const directory = await mkdtemp(
		join(tmpdir(), "daiw-native-device-refresh-"),
	);
	const binary = join(directory, "fake-daiw-play");
	await writeFile(
		binary,
		`#!/usr/bin/env node\nif (process.argv.includes("--list-output-devices")) process.stdout.write(JSON.stringify({ v: 1, devices: [{ index: 0, name: "Test output", nameSha256: "${"a".repeat(64)}", isDefault: true, supported: true }] }));\n`,
	);
	await chmod(binary, 0o755);
	const { controller } = await fixture(t, { binary });
	t.after(() => rm(directory, { recursive: true, force: true }));
	const first = await controller.listDevices("client-1");
	const second = await controller.listDevices("client-1");
	// When / Then
	await assert.rejects(
		controller.prepare("client-1", {
			projectId: "project-1",
			expectedRevision: "revision-1",
			deviceToken: first.devices[0].token,
		}),
		{ code: "InvalidDevice" },
	);
	await controller.releaseClient("client-1");
	await assert.rejects(
		controller.prepare("client-1", {
			projectId: "project-1",
			expectedRevision: "revision-1",
			deviceToken: second.devices[0].token,
		}),
		{ code: "InvalidDevice" },
	);
});

test("controller close invalidates tokens for clients that only listed devices", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`process.stdout.write(JSON.stringify({ v: 1, devices: [{ index: 0, name: "Listed", nameSha256: "${"d".repeat(64)}", isDefault: true, supported: true }] }));`,
	);
	const { controller, renders } = await fixture(t, { binary });
	const listed = await controller.listDevices("client-1");
	// When
	await controller.close();
	// Then
	await assert.rejects(
		controller.prepare("client-1", {
			projectId: "project-1",
			expectedRevision: "revision-1",
			deviceToken: listed.devices[0].token,
		}),
		{ code: "InvalidDevice" },
	);
	assert.equal(renders(), 0);
});

test("controller closes a startup worker when preparation is released", async (t) => {
	// Given
	const marker = join(
		tmpdir(),
		`daiw-native-closed-${process.pid}-${Date.now()}`,
	);
	const pidFile = `${marker}.pid`;
	const { binary } = await fakeWorker(
		t,
		`const fs = require("node:fs");
fs.writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));
setTimeout(() => process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n"), 100);
process.stdin.on("data", (chunk) => {
  const command = JSON.parse(chunk.toString().trim());
  if (command.type === "shutdown") {
    fs.writeFileSync(${JSON.stringify(marker)}, "closed");
    process.exit(0);
  }
});`,
	);
	const { controller } = await fixture(t, { binary });
	const preparation = controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	assert.equal(
		await waitFor(() =>
			import("node:fs").then(({ existsSync }) => existsSync(pidFile)),
		),
		true,
	);
	// When
	await controller.releaseClient("client-1");
	// Then
	await assert.rejects(preparation, { code: "Superseded" });
	assert.equal(
		await waitFor(() =>
			import("node:fs").then(({ existsSync }) => existsSync(marker)),
		),
		true,
	);
	t.after(async () => {
		await rm(marker, { force: true });
		await rm(pidFile, { force: true });
	});
});

test("latest overlapping preparation owns the session while the prior worker closes slowly", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`const state = { status: "stopped", positionFrame: 0, frames: 64, loop: null, stats: { underrunCallbacks: 0, staleBlocks: 0, deviceErrors: 0 } };
process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n");
process.stdin.on("data", (chunk) => {
  for (const line of chunk.toString().trim().split("\\n")) {
    const command = JSON.parse(line);
    const respond = () => process.stdout.write(JSON.stringify({ v: 1, type: "response", id: command.id, ok: true, state }) + "\\n");
    if (command.type === "shutdown") setTimeout(() => { respond(); process.exit(0); }, 100);
    else respond();
  }
});`,
	);
	const { controller } = await fixture(t, { binary });
	const request = { projectId: "project-1", expectedRevision: "revision-1" };
	const initial = await controller.prepare("client-1", request);
	// When
	const older = controller.prepare("client-1", request);
	const latest = controller.prepare("client-1", request);
	const [olderResult, latestResult] = await Promise.allSettled([older, latest]);
	// Then
	assert.equal(olderResult.status, "rejected");
	assert.equal(olderResult.reason.code, "Superseded");
	assert.equal(latestResult.status, "fulfilled");
	await assert.rejects(
		controller.command("client-1", initial.sessionToken, { type: "state" }),
		{ code: "InvalidSession" },
	);
	const state = await controller.command(
		"client-1",
		latestResult.value.sessionToken,
		{ type: "state" },
	);
	assert.equal(state.status, "stopped");
});

test("idle worker exit emits a tagged fault, clears the dead session, and permits retry", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n");
setTimeout(() => process.exit(7), 50);`,
	);
	const { controller, states } = await fixture(t, { binary });
	const request = { projectId: "project-1", expectedRevision: "revision-1" };
	const prepared = await controller.prepare("client-1", request);
	// When
	const faulted = await waitFor(() =>
		states.some((event) => event.fault?.code === "WorkerClosed"),
	);
	// Then
	assert.equal(faulted, true);
	await assert.rejects(
		controller.command("client-1", prepared.sessionToken, { type: "state" }),
		{ code: "InvalidSession" },
	);
	const retried = await controller.prepare("client-1", request);
	assert.notEqual(retried.sessionToken, prepared.sessionToken);
});

test("controller rechecks the project revision after the worker becomes ready", async (t) => {
	// Given
	const { binary } = await fakeWorker(
		t,
		`setTimeout(() => process.stdout.write(JSON.stringify({ v: 1, type: "ready", frames: 64 }) + "\\n"), 100);
process.stdin.on("data", (chunk) => {
  const command = JSON.parse(chunk.toString().trim());
  if (command.type === "shutdown") process.exit(0);
});`,
	);
	const { controller } = await fixture(t, {
		binary,
		revisionAtRead: (read, current) => (read < 3 ? current : "revision-2"),
	});
	const preparation = controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	// When / Then
	await assert.rejects(preparation, { code: "StaleRevision" });
});

test("controller releases a client and ignores late worker output", async (t) => {
	// Given
	const { controller } = await fixture(t);
	const prepared = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	// When
	await controller.releaseClient("client-1");
	// Then
	await assert.rejects(
		controller.command("client-1", prepared.sessionToken, { type: "play" }),
		{ code: "InvalidSession" },
	);
});

test("controller release lets the worker exit after graceful shutdown", async (t) => {
	// Given
	const { controller } = await fixture(t);
	const prepared = await controller.prepare("client-1", {
		projectId: "project-1",
		expectedRevision: "revision-1",
	});
	const startedAt = performance.now();
	// When
	await controller.release("client-1", prepared.sessionToken);
	// Then
	assert.ok(performance.now() - startedAt < 250);
});
