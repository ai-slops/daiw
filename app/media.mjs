import { createHash, randomUUID } from "node:crypto";
import { mkdir, open, readFile, rename, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";
import { beatAtSeconds, secondsAtBeat } from "../shared/timing.mjs";
import { ServiceError } from "./errors.mjs";
import { RenderOrchestrator, renderDurationSeconds } from "./media-render.mjs";
import { RenderCache } from "./render-cache.mjs";
import { SourceCache } from "./source-cache.mjs";

function missing(error) {
	return error instanceof Error && "code" in error && error.code === "ENOENT";
}

export function importDurationBeats(startBeat, sourceSeconds, timing) {
	const duration =
		beatAtSeconds(secondsAtBeat(startBeat, timing) + sourceSeconds, timing) -
		startBeat;
	return Math.ceil(duration * 960 - 1e-9) / 960;
}

export { renderDurationSeconds };

export class MediaStore {
	#renderQueue = Promise.resolve();
	constructor({ directory, binary, service }) {
		this.directory = resolve(directory);
		this.binary = resolve(binary);
		this.service = service;
		this.rendering = this.#createRendering();
	}
	#createRendering() {
		return new RenderOrchestrator({
			binary: this.binary,
			service: this.service,
			sources: this.sources,
			renders: this.renders,
			rendererFingerprint: this.rendererFingerprint,
		});
	}
	async initialize() {
		await Promise.all(
			["objects", "cache/audio", "cache/render"].map((path) =>
				mkdir(join(this.directory, path), { recursive: true }),
			),
		);
		this.rendererFingerprint = createHash("sha256")
			.update(await readFile(this.binary))
			.digest("hex");
		this.sources = new SourceCache({
			directory: this.directory,
			binary: this.binary,
			database: this.service.store.db,
			rendererFingerprint: this.rendererFingerprint,
		});
		this.renders = new RenderCache({
			directory: this.directory,
			database: this.service.store.db,
		});
		this.rendering = this.#createRendering();
		await Promise.all([this.sources.initialize(), this.renders.initialize()]);
	}
	async #atomic(path, bytes) {
		await mkdir(dirname(path), { recursive: true });
		const temporary = `${path}.${randomUUID()}.tmp`;
		const handle = await open(temporary, "wx");
		try {
			await handle.writeFile(bytes);
			await handle.sync();
		} finally {
			await handle.close();
		}
		await rename(temporary, path);
		if (process.platform !== "win32") {
			const parent = await open(dirname(path), "r");
			try {
				await parent.sync();
			} finally {
				await parent.close();
			}
		}
	}
	async #contentAddressed(path, bytes) {
		try {
			await stat(path);
			return;
		} catch (error) {
			if (!missing(error)) throw error;
		}
		await this.#atomic(path, bytes);
	}
	async importAudio(projectId, form, idempotencyKey = randomUUID()) {
		z.string().min(1).max(160).parse(idempotencyKey);
		const file = form.get("file");
		if (!(file instanceof File))
			throw new ServiceError(
				"InvalidMedia",
				"Select a WAV, AIFF, or FLAC audio file.",
			);
		if (file.size > 20 * 1024 * 1024)
			throw new ServiceError(
				"ResourceLimit",
				"Audio imports are limited to 20 MB in this slice.",
				413,
			);
		const fields = z
			.object({
				trackId: z.string().min(1),
				start: z.coerce.number().min(0).max(100000),
				expectedRevision: z.string().min(1),
			})
			.parse({
				trackId: form.get("trackId"),
				start: form.get("start") ?? 0,
				expectedRevision: form.get("expectedRevision"),
			});
		const current = this.service.store.get(projectId);
		if (
			current.revision !== fields.expectedRevision &&
			!this.service.store.hasRequest(projectId, idempotencyKey)
		)
			throw new ServiceError(
				"StaleRevision",
				"Project changed before import. Retry from the latest revision.",
				409,
			);
		const state = this.service.store.getRevision(
			projectId,
			fields.expectedRevision,
		);
		const bytes = Buffer.from(await file.arrayBuffer());
		const hash = createHash("sha256").update(bytes).digest("hex");
		await this.#contentAddressed(join(this.directory, "objects", hash), bytes);
		const decoded = await this.sources.build(hash);
		if (decoded.frames / decoded.sampleRate > 180)
			throw new ServiceError(
				"ResourceLimit",
				"Import must be shorter than 3 minutes.",
				413,
			);
		const asset = {
			id: `asset-${hash}`,
			hash,
			name: file.name.slice(0, 200),
			frames: decoded.frames,
			sampleRate: decoded.sampleRate,
			channels: decoded.channels,
		};
		this.service.store.db
			.prepare("INSERT OR IGNORE INTO assets VALUES (?,?,?,?)")
			.run(asset.id, hash, asset.name, JSON.stringify(asset));
		const duration = importDurationBeats(
			fields.start,
			decoded.frames / decoded.sampleRate,
			state.tempoMap ?? state.tempo,
		);
		const regionId = createHash("sha256")
			.update(`${projectId}:${idempotencyKey}`)
			.digest("hex");
		return this.service.execute({
			projectId,
			method: "change.apply",
			idempotencyKey,
			params: {
				expectedRevision: fields.expectedRevision,
				command: {
					type: "addAudioRegion",
					asset,
					region: {
						id: regionId,
						trackId: fields.trackId,
						assetId: asset.id,
						start: fields.start,
						duration,
						gain: 0,
						sourceOffset: 0,
					},
				},
			},
		});
	}
	render(projectId, proposalId, expectedRevision, signal) {
		const next = this.#renderQueue.then(() =>
			this.rendering.render({
				projectId,
				proposalId,
				expectedRevision,
				signal,
			}),
		);
		this.#renderQueue = next.then(
			() => undefined,
			() => undefined,
		);
		return next;
	}
	reopenRender(key, fileIdentity) {
		return this.renders.reopenVerified(key, fileIdentity);
	}
	async waveform(projectId, assetId) {
		const state = this.service.store.get(projectId);
		const reference = state.assets.find((asset) => asset.id === assetId);
		if (!reference)
			throw new ServiceError(
				"MissingAsset",
				"This source is not part of the project.",
				404,
			);
		const decoded = await this.sources.get(reference);
		return {
			peaks: decoded.peaks128,
			frames: decoded.frames,
			sampleRate: decoded.sampleRate,
			channels: decoded.channels,
		};
	}
}
