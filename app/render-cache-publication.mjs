import { randomUUID } from "node:crypto";
import { open, readdir, rename, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { ServiceError } from "./errors.mjs";
import { fileIdentity } from "./render-cache-identity.mjs";
import { MAX_WAV_FILE_BYTES, verifyWavFile } from "./wav-format.mjs";

const CACHE_BUDGET_BYTES = 2 * 1024 * 1024 * 1024;

async function directoryBytes(root) {
	let total = 0;
	for (const entry of await readdir(root, { withFileTypes: true })) {
		if (entry.isFile()) total += (await stat(join(root, entry.name))).size;
	}
	return total;
}

export async function publishRender(context, request) {
	const { database, cacheRoot, tempRoot, pathFor, assertRoots, realManaged } =
		context;
	const { key, projectId, revision, profile, produce } = request;
	await assertRoots();
	const used =
		(await directoryBytes(cacheRoot)) + (await directoryBytes(tempRoot));
	if (used + MAX_WAV_FILE_BYTES > CACHE_BUDGET_BYTES) {
		throw new ServiceError(
			"ResourceLimit",
			"Render cache capacity is exhausted.",
			413,
		);
	}
	const temporary = resolve(tempRoot, `${key}.${randomUUID()}.tmp`);
	const path = pathFor(key);
	let handle;
	let published = false;
	let ownsTemporary = true;
	try {
		const descriptor = await produce(temporary);
		const temporaryHandle = await open(
			await realManaged(tempRoot, temporary),
			"r+",
		);
		try {
			await verifyWavFile(temporaryHandle, descriptor);
			await temporaryHandle.sync();
		} finally {
			await temporaryHandle.close();
		}
		await assertRoots();
		await rename(temporary, path);
		ownsTemporary = false;
		published = true;
		if (process.platform !== "win32") {
			const parent = await open(dirname(path), "r");
			try {
				await parent.sync();
			} finally {
				await parent.close();
			}
		}
		const managedPath = await realManaged(cacheRoot, path);
		handle = await open(managedPath, "r");
		const analysis = await verifyWavFile(handle, descriptor);
		database.exec("BEGIN IMMEDIATE");
		try {
			database
				.prepare(`INSERT INTO render_cache_files VALUES (?,?,?,?,?,?,?)
          ON CONFLICT(key) DO UPDATE SET format=excluded.format,sample_rate=excluded.sample_rate,channels=excluded.channels,frames=excluded.frames,byte_length=excluded.byte_length,sha256=excluded.sha256`)
				.run(
					key,
					descriptor.format,
					descriptor.sampleRate,
					descriptor.channels,
					descriptor.frames,
					descriptor.byteLength,
					descriptor.sha256,
				);
			database
				.prepare("INSERT OR REPLACE INTO renders VALUES (?,?,?,?,?)")
				.run(key, projectId, revision, profile, new Date().toISOString());
			database.exec("COMMIT");
		} catch (error) {
			database.exec("ROLLBACK");
			throw error;
		}
		return {
			handle,
			managedPath,
			descriptor,
			analysis,
			fileIdentity: fileIdentity(await handle.stat({ bigint: true })),
		};
	} catch (error) {
		await handle?.close();
		if (published) await rm(path, { force: true });
		throw error;
	} finally {
		if (ownsTemporary) await rm(temporary, { force: true });
	}
}
