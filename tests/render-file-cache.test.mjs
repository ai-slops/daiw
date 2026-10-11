import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync } from "node:fs";
import {
	mkdir,
	mkdtemp,
	open,
	readFile,
	rename,
	rm,
	symlink,
	writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { RenderCache } from "../app/render-cache.mjs";
import {
	InvalidWavCacheError,
	parseRenderDescriptor,
	verifyWavFile,
} from "../app/wav-format.mjs";

function wav(frames = 1) {
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
	return bytes;
}

test("strict render descriptors and the same open handle verify a canonical WAV", async (t) => {
	// Given a canonical stereo PCM16 WAV and its renderer descriptor.
	const directory = await mkdtemp(join(tmpdir(), "daiw-wav-verify-"));
	t.after(() => rm(directory, { recursive: true }));
	const path = join(directory, "render.wav");
	const bytes = wav(2);
	await writeFile(path, bytes);
	const descriptor = parseRenderDescriptor({
		format: "daiw-wav-pcm16-v1",
		sampleRate: 48000,
		channels: 2,
		frames: 2,
		byteLength: bytes.length,
		sha256: createHash("sha256").update(bytes).digest("hex"),
	});
	const handle = await open(path, "r");
	t.after(() => handle.close());
	// When verification reads the file, then it accepts the exact descriptor without replacing the handle.
	assert.deepEqual(await verifyWavFile(handle, descriptor), {
		pcmRms: 0,
		pcmPeak: 0,
	});
});

test("PCM16 analysis handles signed samples split across odd scan chunks", async (t) => {
	// Given known normalized PCM16 values whose bytes cross odd stream boundaries.
	const directory = await mkdtemp(join(tmpdir(), "daiw-wav-analysis-"));
	t.after(() => rm(directory, { recursive: true }));
	const path = join(directory, "render.wav");
	const samples = [-32768, -16384, 0, 16384, 32767, -1];
	const bytes = wav(3);
	samples.forEach((sample, index) => {
		bytes.writeInt16LE(sample, 44 + index * 2);
	});
	await writeFile(path, bytes);
	const descriptor = parseRenderDescriptor({
		format: "daiw-wav-pcm16-v1",
		sampleRate: 48000,
		channels: 2,
		frames: 3,
		byteLength: bytes.length,
		sha256: createHash("sha256").update(bytes).digest("hex"),
	});
	const file = await open(path, "r");
	t.after(() => file.close());
	const handle = {
		stat: () => file.stat(),
		read: (...args) => file.read(...args),
		async *createReadStream() {
			yield bytes.subarray(0, 45);
			yield bytes.subarray(45, 48);
			yield bytes.subarray(48);
		},
	};
	// When verification hashes and analyzes the bounded file scan.
	const analysis = await verifyWavFile(handle, descriptor);
	// Then RMS and peak use signed int16 values normalized by 32768.
	const normalized = samples.map((sample) => sample / 32768);
	const expectedRms = Math.sqrt(
		normalized.reduce((sum, sample) => sum + sample * sample, 0) /
			normalized.length,
	);
	assert.equal(analysis.pcmRms, expectedRms);
	assert.equal(analysis.pcmPeak, 1);
});

test("same-length WAV payload and header corruption are rejected", async (t) => {
	// Given a descriptor for pristine bytes.
	const directory = await mkdtemp(join(tmpdir(), "daiw-wav-corrupt-"));
	t.after(() => rm(directory, { recursive: true }));
	const path = join(directory, "render.wav");
	const pristine = wav(2);
	const descriptor = parseRenderDescriptor({
		format: "daiw-wav-pcm16-v1",
		sampleRate: 48000,
		channels: 2,
		frames: 2,
		byteLength: pristine.length,
		sha256: createHash("sha256").update(pristine).digest("hex"),
	});
	// When bytes change without changing length, then both header and payload corruption fail verification.
	for (const offset of [0, 44]) {
		const corrupt = Buffer.from(pristine);
		corrupt[offset] ^= 0xff;
		await writeFile(path, corrupt);
		const handle = await open(path, "r");
		await assert.rejects(verifyWavFile(handle, descriptor));
		await handle.close();
	}
});

test("initial render-root symlink escape is rejected before outside cleanup", async (t) => {
	// Given a managed render root redirected outside the canonical data directory.
	const root = await mkdtemp(join(tmpdir(), "daiw-render-root-"));
	t.after(() => rm(root, { recursive: true }));
	const directory = join(root, "data");
	const outside = join(root, "outside");
	await Promise.all([
		mkdir(join(directory, "cache"), { recursive: true }),
		mkdir(outside),
	]);
	const outsideFile = join(outside, `${"a".repeat(64)}.wav`);
	await writeFile(outsideFile, "outside-owned-by-someone-else");
	await symlink(outside, join(directory, "cache/render"));
	const database = { prepare: () => ({ get: () => undefined }) };
	// When startup validates roots, then it rejects rather than adopting and cleaning outside.
	await assert.rejects(new RenderCache({ directory, database }).initialize(), {
		code: "CorruptRender",
	});
	assert.equal(
		await readFile(outsideFile, "utf8"),
		"outside-owned-by-someone-else",
	);
});

function cacheDatabase() {
	const database = new DatabaseSync(":memory:");
	database.exec(`CREATE TABLE render_cache_files (key TEXT PRIMARY KEY, format TEXT, sample_rate INTEGER, channels INTEGER, frames INTEGER, byte_length INTEGER, sha256 TEXT);
    CREATE TABLE renders (key TEXT PRIMARY KEY, project_id TEXT, revision TEXT, profile TEXT, created_at TEXT);`);
	return database;
}

test("post-commit temp-path failure cannot replace a successful publication", async (t) => {
	// Given a database hook recreating the former temp path as a directory after commit.
	const directory = await mkdtemp(join(tmpdir(), "daiw-render-commit-"));
	t.after(() => rm(directory, { recursive: true }));
	const database = cacheDatabase();
	t.after(() => database.close());
	let temporary;
	const injected = {
		prepare: (sql) => database.prepare(sql),
		exec: (sql) => {
			database.exec(sql);
			if (sql === "COMMIT") mkdirSync(temporary);
		},
	};
	const cache = new RenderCache({ directory, database: injected });
	await cache.initialize();
	const key = "b".repeat(64);
	const bytes = wav(2);
	const descriptor = parseRenderDescriptor({
		format: "daiw-wav-pcm16-v1",
		sampleRate: 48000,
		channels: 2,
		frames: 2,
		byteLength: bytes.length,
		sha256: createHash("sha256").update(bytes).digest("hex"),
	});
	// When publication commits, then transferred temp ownership causes no post-commit removal.
	const built = await cache.build({
		key,
		projectId: "project-1",
		revision: "revision-1",
		profile: "profile-1",
		produce: async (path) => {
			temporary = path;
			await writeFile(path, bytes);
			return descriptor;
		},
	});
	t.after(() => built.handle.close());
	assert.equal(
		database.prepare("SELECT count(*) AS total FROM render_cache_files").get()
			.total,
		1,
	);
	assert.equal(
		database.prepare("SELECT count(*) AS total FROM renders").get().total,
		1,
	);
	assert.equal((await built.handle.stat()).size, bytes.length);
	assert.equal(
		built.managedPath,
		join(directory, "cache/render", `${key}.wav`),
	);
});

test("metadata invalidation rolls back both rows and preserves the verification error", async (t) => {
	// Given corrupt bytes with both metadata rows and a failure on the second delete.
	const directory = await mkdtemp(join(tmpdir(), "daiw-render-invalidate-"));
	t.after(() => rm(directory, { recursive: true }));
	const database = cacheDatabase();
	t.after(() => database.close());
	const cache = new RenderCache({ directory, database });
	await cache.initialize();
	const key = "c".repeat(64);
	const pristine = wav(2);
	const corrupt = Buffer.from(pristine);
	corrupt[44] ^= 0xff;
	const hash = createHash("sha256").update(pristine).digest("hex");
	database
		.prepare("INSERT INTO render_cache_files VALUES (?,?,?,?,?,?,?)")
		.run(key, "daiw-wav-pcm16-v1", 48000, 2, 2, pristine.length, hash);
	database
		.prepare("INSERT INTO renders VALUES (?,?,?,?,?)")
		.run(key, "project-1", "revision-1", "profile-1", "now");
	database.exec(
		"CREATE TRIGGER fail_render_delete BEFORE DELETE ON renders BEGIN SELECT RAISE(ABORT, 'injected second delete failure'); END;",
	);
	await writeFile(join(directory, "cache/render", `${key}.wav`), corrupt);
	// When invalidation fails midway, then rollback keeps metadata atomic and the WAV error wins.
	await assert.rejects(
		cache.open(key),
		(error) => error instanceof InvalidWavCacheError,
	);
	assert.equal(
		database.prepare("SELECT count(*) AS total FROM render_cache_files").get()
			.total,
		1,
	);
	assert.equal(
		database.prepare("SELECT count(*) AS total FROM renders").get().total,
		1,
	);
});

test("verified reopen compares exact same-FD identity without rehashing a replacement", async (t) => {
	// Given a published render and its post-verification file identity.
	const directory = await mkdtemp(join(tmpdir(), "daiw-render-reopen-"));
	t.after(() => rm(directory, { recursive: true }));
	const database = cacheDatabase();
	t.after(() => database.close());
	const cache = new RenderCache({ directory, database });
	await cache.initialize();
	const key = "d".repeat(64);
	const bytes = wav(2);
	const descriptor = parseRenderDescriptor({
		format: "daiw-wav-pcm16-v1",
		sampleRate: 48000,
		channels: 2,
		frames: 2,
		byteLength: bytes.length,
		sha256: createHash("sha256").update(bytes).digest("hex"),
	});
	const built = await cache.build({
		key,
		projectId: "project-1",
		revision: "revision-1",
		profile: "profile-1",
		produce: async (path) => {
			await writeFile(path, bytes);
			return descriptor;
		},
	});
	await built.handle.close();
	// When the unchanged file and then a same-length replacement are reopened by server-owned key.
	const unchanged = await cache.reopenVerified(key, built.fileIdentity);
	assert.ok(unchanged);
	await unchanged.close();
	const replacement = Buffer.from(bytes);
	replacement[44] ^= 0xff;
	const path = join(directory, "cache/render", `${key}.wav`);
	await writeFile(`${path}.replacement`, replacement);
	await rename(`${path}.replacement`, path);
	// Then identity allows the original and rejects the replacement without returning its handle.
	assert.equal(await cache.reopenVerified(key, built.fileIdentity), undefined);
});
