import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, open, rm, writeFile } from "node:fs/promises";
import { createServer, get } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { ServiceError } from "../app/errors.mjs";
import { createHandler } from "../app/http.mjs";

async function serverFixture(t, media) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-http-render-"));
	await writeFile(join(directory, "index.html"), "test");
	const service = { store: { list: () => [] } };
	const server = createServer(
		createHandler({ service, media, staticRoot: directory }),
	);
	server.listen(0, "127.0.0.1");
	await once(server, "listening");
	const address = server.address();
	assert.ok(address && typeof address === "object");
	t.after(async () => {
		server.closeAllConnections();
		await new Promise((done) => server.close(done));
		await rm(directory, { recursive: true });
	});
	return `http://127.0.0.1:${address.port}`;
}

test("aborted backpressured transfer closes its handle and leaves the next download healthy", async (t) => {
	// Given a large valid WAV served through a tracked file handle.
	const directory = await mkdtemp(join(tmpdir(), "daiw-http-wav-"));
	t.after(() => rm(directory, { recursive: true }));
	const frames = 2_000_000;
	const bytes = Buffer.alloc(44 + frames * 4);
	bytes.write("RIFF", 0);
	bytes.writeUInt32LE(bytes.length - 8, 4);
	bytes.write("WAVEfmt ", 8);
	bytes.writeUInt32LE(16, 16);
	bytes.writeUInt16LE(1, 20);
	bytes.writeUInt16LE(2, 22);
	bytes.writeUInt32LE(48000, 24);
	bytes.writeUInt32LE(192000, 28);
	bytes.writeUInt16LE(4, 32);
	bytes.writeUInt16LE(16, 34);
	bytes.write("data", 36);
	bytes.writeUInt32LE(frames * 4, 40);
	const path = join(directory, "large.wav");
	await writeFile(path, bytes);
	let closes = 0;
	let noteFirstClose;
	let noteSecondClose;
	const firstClose = new Promise((resolve) => {
		noteFirstClose = resolve;
	});
	const secondClose = new Promise((resolve) => {
		noteSecondClose = resolve;
	});
	const media = {
		render: async () => {
			const file = await open(path, "r");
			return {
				handle: {
					createReadStream: (options) => file.createReadStream(options),
					close: async () => {
						closes += 1;
						await file.close();
						if (closes === 1) noteFirstClose();
						else noteSecondClose();
					},
				},
				revision: "revision-1",
				key: "a".repeat(64),
				descriptor: {
					byteLength: bytes.length,
					sha256: createHash("sha256").update(bytes).digest("hex"),
				},
			};
		},
	};
	const base = await serverFixture(t, media);
	// When one client disconnects under backpressure and the next reads the full body.
	await new Promise((resolve, reject) => {
		const request = get(
			`${base}/api/projects/project-1/render.wav`,
			(response) => {
				response.once("data", () => request.destroy());
				response.once("close", resolve);
				response.once("error", (error) => {
					if (error.code !== "ECONNRESET") reject(error);
				});
			},
		);
		request.once("error", (error) => {
			if (error.code !== "ECONNRESET") reject(error);
		});
	});
	await firstClose;
	const healthy = await fetch(`${base}/api/projects/project-1/render.wav`);
	const downloaded = Buffer.from(await healthy.arrayBuffer());
	await secondClose;
	// Then both handles close, the queue remains usable, and byte parity is exact.
	assert.equal(closes, 2);
	assert.equal(
		createHash("sha256").update(downloaded).digest("hex"),
		createHash("sha256").update(bytes).digest("hex"),
	);
});

test("disconnect before headers aborts the owned render signal", async (t) => {
	// Given a render operation waiting only on its request-owned abort signal.
	let cancelled = false;
	let noteStarted;
	let noteCancelled;
	const started = new Promise((resolve) => {
		noteStarted = resolve;
	});
	const cancellation = new Promise((resolve) => {
		noteCancelled = resolve;
	});
	const media = {
		render: async (_projectId, _proposalId, _revision, signal) =>
			new Promise((_resolve, reject) => {
				noteStarted();
				signal.addEventListener(
					"abort",
					() => {
						cancelled = true;
						noteCancelled();
						reject(new ServiceError("RequestAborted", "cancelled", 499));
					},
					{ once: true },
				);
			}),
	};
	const base = await serverFixture(t, media);
	// When the client disconnects before response headers, then cancellation reaches the renderer.
	const request = get(`${base}/api/projects/project-1/render.wav`);
	request.once("error", () => undefined);
	await started;
	const closed = new Promise((resolve) => request.once("close", resolve));
	request.destroy();
	await closed;
	await cancellation;
	assert.equal(cancelled, true);
});

test("disconnect resolved by media with a handle closes it without starting a stream", async (t) => {
	// Given media that finishes with a handle only after observing the disconnect.
	let noteStarted;
	const started = new Promise((resolve) => {
		noteStarted = resolve;
	});
	let closes = 0;
	let streams = 0;
	let noteHandleClosed;
	const handleClosed = new Promise((resolve) => {
		noteHandleClosed = resolve;
	});
	const media = {
		render: async (_projectId, _proposalId, _revision, signal) =>
			new Promise((resolve) => {
				noteStarted();
				signal.addEventListener(
					"abort",
					() =>
						resolve({
							handle: {
								close: async () => {
									closes += 1;
									noteHandleClosed();
								},
								createReadStream: () => {
									streams += 1;
									throw new Error("stream must not start");
								},
							},
							revision: "revision-1",
							key: "a".repeat(64),
							descriptor: { byteLength: 44, sha256: "0".repeat(64) },
						}),
					{ once: true },
				);
			}),
	};
	const base = await serverFixture(t, media);
	const request = get(`${base}/api/projects/project-1/render.wav`);
	request.once("error", () => undefined);
	await started;
	const closed = new Promise((resolve) => request.once("close", resolve));
	request.destroy();
	await closed;
	await handleClosed;
	// Then the post-render abort gate closes the handle before headers or pipeline creation.
	assert.equal(closes, 1);
	assert.equal(streams, 0);
});

test("render failures preserve stale, capacity, and infrastructure HTTP statuses", async (t) => {
	// Given distinct typed failures at the render boundary.
	const failures = {
		stale: new ServiceError("StaleRevision", "stale", 409),
		capacity: new ServiceError("ResourceLimit", "full", 413),
		infrastructure: new ServiceError("RenderInfrastructure", "disk", 500),
	};
	const media = {
		render: async (projectId) => {
			throw failures[projectId];
		},
	};
	const base = await serverFixture(t, media);
	// When each request fails before headers, then JSON status mapping remains exact.
	for (const [projectId, status] of [
		["stale", 409],
		["capacity", 413],
		["infrastructure", 500],
	]) {
		const response = await fetch(
			`${base}/api/projects/${projectId}/render.wav`,
		);
		assert.equal(response.status, status);
		assert.equal((await response.json()).error.code, failures[projectId].code);
	}
});
