import { createHash } from "node:crypto";
import { z } from "zod";
import { secondsAtBeat } from "../shared/timing.mjs";
import { ServiceError } from "./errors.mjs";
import { parseRenderSnapshot } from "./media-snapshot.mjs";
import { parseRenderDescriptor } from "./wav-format.mjs";
import { runBinary } from "./worker.mjs";

const MAX_RENDER_SOURCES = 64;
const MAX_RENDER_SOURCE_BYTES = 1024 * 1024 * 1024;
const DESCRIPTOR_STDOUT_LIMIT = 16 * 1024;
const RENDER_PROFILE = "daiw-render-file-backed-v1-48000";

function aborted() {
	return new ServiceError(
		"RequestAborted",
		"Audio rendering was cancelled.",
		499,
	);
}

function throwIfAborted(signal) {
	if (signal?.aborted) throw aborted();
}

function waitForAbort(operation, signal) {
	if (!signal) return operation;
	if (signal.aborted) return Promise.reject(aborted());
	return new Promise((resolve, reject) => {
		let settled = false;
		const finish = (callback, value) => {
			if (settled) return;
			settled = true;
			signal.removeEventListener("abort", onAbort);
			callback(value);
		};
		const onAbort = () => finish(reject, aborted());
		signal.addEventListener("abort", onAbort, { once: true });
		operation.then(
			(value) => finish(resolve, value),
			(error) => finish(reject, error),
		);
		if (signal.aborted) onAbort();
	});
}

export function renderDurationSeconds(lengthBeats, timing) {
	return secondsAtBeat(lengthBeats, timing);
}

export class RenderOrchestrator {
	constructor({ binary, service, sources, renders, rendererFingerprint }) {
		this.binary = binary;
		this.service = service;
		this.sources = sources;
		this.renders = renders;
		this.rendererFingerprint = rendererFingerprint;
	}

	async render({ projectId, proposalId, expectedRevision, signal }) {
		throwIfAborted(signal);
		const response = await this.service.execute({
			projectId,
			method: "project.snapshot",
			params: proposalId ? { proposalId } : {},
		});
		throwIfAborted(signal);
		if (!response.ok) {
			throw new ServiceError(response.error.code, response.error.message, 422);
		}
		const snapshot = parseRenderSnapshot(response.result);
		if (expectedRevision && snapshot.revision !== expectedRevision) {
			throw new ServiceError(
				"StaleRevision",
				"The project changed before rendering. Refresh the audition snapshot.",
				409,
			);
		}
		if (
			renderDurationSeconds(
				snapshot.length,
				snapshot.tempoMap ?? snapshot.tempo,
			) > 180
		) {
			throw new ServiceError(
				"ResourceLimit",
				"This renderer supports arrangements up to 3 minutes.",
				413,
			);
		}
		const assetIds = [
			...new Set(snapshot.regions.map((region) => region.assetId)),
		].sort();
		if (assetIds.length > MAX_RENDER_SOURCES) {
			throw new ServiceError(
				"ResourceLimit",
				"Renders support at most 64 distinct audio sources.",
				413,
			);
		}
		const sources = {};
		let totalBytes = 0;
		for (const assetId of assetIds) {
			const reference = response.state.assets.find(
				(asset) => asset.id === assetId,
			);
			if (!reference) {
				throw new ServiceError(
					"MissingAsset",
					"The snapshot refers to an unknown project asset.",
					422,
				);
			}
			const decoded = await waitForAbort(this.sources.get(reference), signal);
			throwIfAborted(signal);
			totalBytes += decoded.byteLength;
			if (totalBytes > MAX_RENDER_SOURCE_BYTES) {
				throw new ServiceError(
					"ResourceLimit",
					"Decoded render sources exceed 1 GB.",
					413,
				);
			}
			sources[assetId] = {
				cacheId: decoded.cacheId,
				sha256: decoded.sha256,
				frames: decoded.frames,
				sampleRate: decoded.sampleRate,
				channels: decoded.channels,
				byteLength: decoded.byteLength,
			};
		}
		const identities = assetIds.map((assetId) => [
			assetId,
			sources[assetId].sha256,
		]);
		const key = createHash("sha256")
			.update(this.rendererFingerprint)
			.update(JSON.stringify(identities))
			.update(JSON.stringify(snapshot))
			.update(RENDER_PROFILE)
			.digest("hex");
		throwIfAborted(signal);
		const existing = await this.renders.open(key);
		if (signal?.aborted) {
			await existing?.handle.close();
			throw aborted();
		}
		if (existing) return { ...existing, revision: snapshot.revision, key };
		const profile = `${RENDER_PROFILE}@${this.rendererFingerprint}`;
		const built = await this.renders.build({
			key,
			projectId,
			revision: snapshot.revision,
			profile,
			produce: async (temporary) => {
				const stdout = await runBinary({
					binary: this.binary,
					args: [
						"--render-to-file",
						"--source-root",
						this.sources.cacheRoot,
						"--output",
						temporary,
					],
					input: JSON.stringify({ snapshot, sources }),
					limit: DESCRIPTOR_STDOUT_LIMIT,
					resourceLimitExitCode: 2,
					infrastructureExitCode: 3,
					signal,
				});
				try {
					return parseRenderDescriptor(JSON.parse(stdout.toString("utf8")));
				} catch (error) {
					if (!(error instanceof z.ZodError) && !(error instanceof SyntaxError)) {
						throw error;
					}
					throw new ServiceError(
						"RenderInfrastructure",
						"Renderer returned an invalid file descriptor.",
						500,
					);
				}
			},
		});
		if (signal?.aborted) {
			await built.handle.close();
			throw aborted();
		}
		return { ...built, revision: snapshot.revision, key };
	}
}
