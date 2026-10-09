import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, open, rename, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { createHandler } from "../app/http.mjs";

function wav(samples) {
	const bytes = Buffer.alloc(44 + samples.length * 2);
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
	bytes.writeUInt32LE(samples.length * 2, 40);
	samples.forEach((sample, index) => {
		bytes.writeInt16LE(sample, 44 + index * 2);
	});
	return bytes;
}

async function fixture(t, samples = [-32768, 0, 16384, -16384, 32767, -1]) {
	const directory = await mkdtemp(join(tmpdir(), "daiw-http-range-"));
	const bytes = wav(samples);
	const path = join(directory, "render.wav");
	await Promise.all([
		writeFile(join(directory, "index.html"), "test"),
		writeFile(path, bytes),
	]);
	const revision = "revision-1";
	const key = "a".repeat(64);
	const sha256 = createHash("sha256").update(bytes).digest("hex");
	let closes = 0;
	let calls = 0;
	let verifiedBytes = 0;
	let rangedBytes = 0;
	const identity = (value) => ({
		dev: value.dev,
		ino: value.ino,
		size: value.size,
		mtimeNs: value.mtimeNs,
		ctimeNs: value.ctimeNs,
	});
	const sameIdentity = (left, right) =>
		left.dev === right.dev &&
		left.ino === right.ino &&
		left.size === right.size &&
		left.mtimeNs === right.mtimeNs &&
		left.ctimeNs === right.ctimeNs;
	const media = {
		render: async (_projectId, proposalId, expectedRevision) => {
			calls += 1;
			const handle = await open(path, "r");
			const fileIdentity = identity(await handle.stat({ bigint: true }));
			verifiedBytes += Number(fileIdentity.size);
			const renderKey = proposalId === "proposal-2" ? "b".repeat(64) : key;
			return {
				handle: {
					createReadStream: (options) => handle.createReadStream(options),
					close: async () => {
						closes += 1;
						await handle.close();
					},
				},
				revision,
				key: renderKey,
				descriptor: {
					format: "daiw-wav-pcm16-v1",
					sampleRate: 48000,
					channels: 2,
					frames: samples.length / 2,
					byteLength: bytes.length,
					sha256,
				},
				analysis: { pcmRms: 0.5, pcmPeak: 1 },
				fileIdentity,
				expectedRevision,
			};
		},
		reopenRender: async (_key, expectedIdentity) => {
			const handle = await open(path, "r");
			if (
				!sameIdentity(
					identity(await handle.stat({ bigint: true })),
					expectedIdentity,
				)
			) {
				await handle.close();
				return undefined;
			}
			return {
				createReadStream: (options) => {
					rangedBytes += options.end - options.start + 1;
					return handle.createReadStream(options);
				},
				close: async () => {
					closes += 1;
					await handle.close();
				},
			};
		},
	};
	const service = { store: { list: () => [], get: () => ({ revision }) } };
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
	return {
		base: `http://127.0.0.1:${address.port}`,
		bytes,
		path,
		revision,
		key,
		sha256,
		closes: () => closes,
		calls: () => calls,
		verifiedBytes: () => verifiedBytes,
		rangedBytes: () => rangedBytes,
	};
}

test("render-info returns exact verified metadata and closes its handle", async (t) => {
	// Given a verified render with ephemeral PCM analysis.
	const host = await fixture(t);
	// When its revision-bound metadata endpoint is requested.
	const response = await fetch(
		`${host.base}/api/projects/project-1/render-info?expectedRevision=${host.revision}`,
	);
	const value = await response.json();
	// Then the exact browser playback contract, identity headers, and cleanup are observable.
	assert.equal(response.status, 200);
	assert.deepEqual(value, {
		format: "daiw-wav-pcm16-v1",
		revision: host.revision,
		renderKey: host.key,
		sampleRate: 48000,
		channels: 2,
		frames: 3,
		byteLength: 56,
		dataOffset: 44,
		bytesPerFrame: 4,
		sha256: host.sha256,
		pcmRms: 0.5,
		pcmPeak: 1,
	});
	assert.equal(response.headers.get("etag"), `"${host.key}"`);
	assert.equal(response.headers.get("x-daiw-revision"), host.revision);
	assert.equal(response.headers.get("x-daiw-sha256"), host.sha256);
	assert.equal(host.closes(), 1);
});

test("render-info full-scans once and warm aligned ranges read only their PCM chunks", async (t) => {
	// Given metadata that completed one full render verification and registered its file identity.
	const host = await fixture(t);
	const metadata = await fetch(
		`${host.base}/api/projects/project-1/render-info?expectedRevision=${host.revision}`,
	);
	assert.equal(metadata.status, 200);
	// When frame-aligned ranges select the first, middle, and last PCM frames.
	for (const [start, end] of [
		[44, 47],
		[48, 51],
		[52, 55],
	]) {
		const response = await fetch(
			`${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}`,
			{ headers: { range: `bytes=${start}-${end}` } },
		);
		const ranged = Buffer.from(await response.arrayBuffer());
		// Then each 206 body and identity/range header is exact.
		assert.equal(response.status, 206);
		assert.deepEqual(ranged, host.bytes.subarray(start, end + 1));
		assert.equal(
			response.headers.get("content-range"),
			`bytes ${start}-${end}/${host.bytes.length}`,
		);
		assert.equal(
			response.headers.get("content-length"),
			String(end - start + 1),
		);
		assert.equal(response.headers.get("content-encoding"), null);
		assert.equal(response.headers.get("etag"), `"${host.key}"`);
	}
	assert.equal(host.closes(), 4);
	assert.equal(host.calls(), 1);
	assert.equal(host.verifiedBytes(), host.bytes.length);
	assert.equal(host.rangedBytes(), 12);
});

test("a same-length render replacement invalidates its lease and returns typed 409", async (t) => {
	// Given metadata registered against the verified file descriptor identity.
	const host = await fixture(t);
	const metadata = await fetch(
		`${host.base}/api/projects/project-1/render-info?expectedRevision=${host.revision}`,
	);
	assert.equal(metadata.status, 200);
	const replacement = Buffer.from(host.bytes);
	replacement[44] ^= 0xff;
	const temporary = `${host.path}.replacement`;
	await writeFile(temporary, replacement);
	await rename(temporary, host.path);
	// When a warm range reopens the server-owned cache path.
	const response = await fetch(
		`${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}`,
		{ headers: { range: "bytes=44-47" } },
	);
	// Then same-size replacement is rejected without a body fallback or second full scan.
	assert.equal(response.status, 409);
	assert.equal((await response.json()).error.code, "RenderChanged");
	assert.equal(host.calls(), 1);
	assert.equal(host.verifiedBytes(), host.bytes.length);
	assert.equal(host.rangedBytes(), 0);
});

test("verified leases never cross project identity", async (t) => {
	// Given one project's metadata lease for a render key shared by the fixture.
	const host = await fixture(t);
	const metadata = await fetch(
		`${host.base}/api/projects/project-1/render-info?expectedRevision=${host.revision}`,
	);
	assert.equal(metadata.status, 200);
	// When another project requests that same revision and key.
	const response = await fetch(
		`${host.base}/api/projects/project-2/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}`,
		{ headers: { range: "bytes=44-47" } },
	);
	// Then it cold-verifies independently rather than borrowing project-1's lease.
	assert.equal(response.status, 206);
	assert.equal(host.calls(), 2);
	assert.equal(host.verifiedBytes(), host.bytes.length * 2);
});

test("ranges are capped at 24000 PCM frames", async (t) => {
	// Given a render containing more than the maximum range span.
	const host = await fixture(t, new Array(48_002).fill(0));
	const url = `${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}`;
	// When the maximum span and one frame beyond it are requested.
	const maximum = await fetch(url, {
		headers: { range: `bytes=44-${44 + 24_000 * 4 - 1}` },
	});
	const oversized = await fetch(url, {
		headers: { range: `bytes=44-${44 + 24_001 * 4 - 1}` },
	});
	// Then the exact maximum succeeds and the larger request is a typed 416.
	assert.equal(maximum.status, 206);
	assert.equal((await maximum.arrayBuffer()).byteLength, 24_000 * 4);
	assert.equal(oversized.status, 416);
	assert.equal((await oversized.json()).error.code, "InvalidRange");
});

test("range requests reject missing or wrong render identity before audio", async (t) => {
	// Given a render whose proposal changes its render key.
	const host = await fixture(t);
	const cases = [
		[`${host.base}/api/projects/project-1/render.wav`, 400, "InvalidRequest"],
		[
			`${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}`,
			400,
			"InvalidRequest",
		],
		[
			`${host.base}/api/projects/project-1/render.wav?expectedRevision=wrong&renderKey=${host.key}`,
			409,
			"StaleRevision",
		],
		[
			`${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${"c".repeat(64)}`,
			409,
			"RenderIdentityMismatch",
		],
		[
			`${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}&proposalId=proposal-2`,
			409,
			"RenderIdentityMismatch",
		],
	];
	// When required identities are absent or differ.
	for (const [url, status, code] of cases) {
		const response = await fetch(url, { headers: { range: "bytes=44-47" } });
		assert.equal(response.status, status);
		assert.equal((await response.json()).error.code, code);
	}
	// Then requests rejected after opening still expose actual identity and close their handles.
	assert.equal(host.calls(), 2);
	assert.equal(host.closes(), 2);
});

test("malformed, multiple, open, suffix, unaligned, and outbound ranges return 416", async (t) => {
	// Given valid strict render identity.
	const host = await fixture(t);
	const url = `${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}`;
	const ranges = [
		"bytes=44-47,48-51",
		"bytes=-4",
		"bytes=44-",
		"bytes=43-46",
		"bytes=45-48",
		"bytes=44-48",
		"bytes=56-59",
		"bytes=52-51",
		"items=44-47",
	];
	// When each unsupported or invalid range is requested.
	for (const range of ranges) {
		const response = await fetch(url, { headers: { range } });
		// Then no 200 fallback occurs and exact unsatisfied/identity headers remain available.
		assert.equal(response.status, 416);
		assert.equal(
			response.headers.get("content-range"),
			`bytes */${host.bytes.length}`,
		);
		assert.equal(response.headers.get("accept-ranges"), "bytes");
		assert.equal(response.headers.get("x-daiw-sha256"), host.sha256);
		assert.equal((await response.json()).error.code, "InvalidRange");
	}
	assert.equal(host.closes(), ranges.length);
});

test("empty PCM renders reject every byte range and render-info requires a revision", async (t) => {
	// Given a canonical header-only WAV.
	const host = await fixture(t, []);
	// When metadata lacks a revision and a range targets nonexistent PCM.
	const missing = await fetch(
		`${host.base}/api/projects/project-1/render-info`,
	);
	const ranged = await fetch(
		`${host.base}/api/projects/project-1/render.wav?expectedRevision=${host.revision}&renderKey=${host.key}`,
		{ headers: { range: "bytes=44-47" } },
	);
	// Then metadata fails before rendering and the range is unsatisfied with cleanup.
	assert.equal(missing.status, 400);
	assert.equal((await missing.json()).error.code, "InvalidRequest");
	assert.equal(ranged.status, 416);
	assert.equal(ranged.headers.get("content-range"), "bytes */44");
	assert.equal(host.calls(), 1);
	assert.equal(host.closes(), 1);
});
