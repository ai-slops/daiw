import { pipeline } from "node:stream/promises";
import { ServiceError } from "./errors.mjs";
import { VerifiedRenderLeases } from "./render-lease.mjs";

function requiredQuery(url, name) {
	const value = url.searchParams.get(name);
	if (!value) {
		throw new ServiceError(
			"InvalidRequest",
			`${name} is required for this request.`,
			400,
		);
	}
	return value;
}

function parsePcmRange(value, byteLength) {
	const match =
		typeof value === "string" ? /^bytes=(\d+)-(\d+)$/.exec(value) : null;
	if (!match) {
		throw new ServiceError(
			"InvalidRange",
			"A single closed byte range is required.",
			416,
		);
	}
	const start = Number(match[1]);
	const end = Number(match[2]);
	if (
		!Number.isSafeInteger(start) ||
		!Number.isSafeInteger(end) ||
		start < 44 ||
		start > end ||
		end >= byteLength ||
		(start - 44) % 4 !== 0 ||
		(end + 1 - 44) % 4 !== 0 ||
		end - start + 1 > 24_000 * 4
	) {
		throw new ServiceError(
			"InvalidRange",
			"The byte range must select complete PCM frames.",
			416,
		);
	}
	return { start, end };
}

function setRenderIdentityHeaders(response, rendered) {
	response.setHeader("ETag", `"${rendered.key}"`);
	response.setHeader("X-DAIW-Revision", rendered.revision);
	response.setHeader("X-DAIW-SHA256", rendered.descriptor.sha256);
	response.setHeader("Accept-Ranges", "bytes");
}

export function createRenderRoute({ service, media, send }) {
	const renderLeases = new VerifiedRenderLeases();
	return async ({ request, response, url, projectId, kind }) => {
		const rangeHeader = request.headers.range;
		const expectedRevision =
			kind === "render-info" || rangeHeader !== undefined
				? requiredQuery(url, "expectedRevision")
				: url.searchParams.get("expectedRevision");
		const expectedRenderKey =
			rangeHeader !== undefined ? requiredQuery(url, "renderKey") : null;
		const proposalId = url.searchParams.get("proposalId");
		const leaseBinding = expectedRenderKey
			? {
					projectId,
					proposalId,
					revision: expectedRevision,
					renderKey: expectedRenderKey,
				}
			: null;
		if (
			rangeHeader !== undefined &&
			service.store.get(projectId).revision !== expectedRevision
		) {
			throw new ServiceError(
				"StaleRevision",
				"The project changed before rendering. Refresh the audition snapshot.",
				409,
			);
		}
		const controller = new AbortController();
		const abort = () => {
			if (!response.writableEnded) controller.abort();
		};
		request.once("aborted", abort);
		response.once("close", abort);
		let rendered;
		let warmLease = false;
		try {
			const lease = leaseBinding ? renderLeases.get(leaseBinding) : undefined;
			if (lease) {
				const handle = await media.reopenRender(lease.key, lease.fileIdentity);
				if (!handle) {
					renderLeases.invalidate(leaseBinding);
					setRenderIdentityHeaders(response, lease);
					throw new ServiceError(
						"RenderChanged",
						"The verified render file changed. Refresh render metadata before requesting audio.",
						409,
					);
				}
				rendered = { ...lease, handle };
				warmLease = true;
			} else {
				rendered = await media.render(
					projectId,
					proposalId,
					expectedRevision,
					controller.signal,
				);
			}
			if (controller.signal.aborted || response.destroyed) {
				throw new ServiceError(
					"RequestAborted",
					"Audio download was cancelled.",
					499,
				);
			}
			setRenderIdentityHeaders(response, rendered);
			if (expectedRevision && rendered.revision !== expectedRevision) {
				throw new ServiceError(
					"StaleRevision",
					"The project changed before rendering. Refresh the audition snapshot.",
					409,
				);
			}
			if (expectedRenderKey && rendered.key !== expectedRenderKey) {
				throw new ServiceError(
					"RenderIdentityMismatch",
					"The requested render identity no longer matches this project snapshot.",
					409,
				);
			}
			if (!warmLease) {
				renderLeases.remember(
					{
						projectId,
						proposalId,
						revision: rendered.revision,
						renderKey: rendered.key,
					},
					rendered,
				);
			}
			if (kind === "render-info") {
				const { descriptor, analysis } = rendered;
				return send(response, 200, {
					format: descriptor.format,
					revision: rendered.revision,
					renderKey: rendered.key,
					sampleRate: descriptor.sampleRate,
					channels: descriptor.channels,
					frames: descriptor.frames,
					byteLength: descriptor.byteLength,
					dataOffset: 44,
					bytesPerFrame: 4,
					sha256: descriptor.sha256,
					pcmRms: analysis.pcmRms,
					pcmPeak: analysis.pcmPeak,
				});
			}
			let start = 0;
			let end = rendered.descriptor.byteLength - 1;
			let status = 200;
			if (rangeHeader !== undefined) {
				response.setHeader(
					"Content-Range",
					`bytes */${rendered.descriptor.byteLength}`,
				);
				({ start, end } = parsePcmRange(
					rangeHeader,
					rendered.descriptor.byteLength,
				));
				response.setHeader(
					"Content-Range",
					`bytes ${start}-${end}/${rendered.descriptor.byteLength}`,
				);
				status = 206;
			}
			response.writeHead(status, {
				"content-type": "audio/wav",
				"content-length": end - start + 1,
				"cache-control": "private, no-cache",
				"Content-Disposition": 'inline; filename="daiw-mix.wav"',
			});
			await pipeline(
				rendered.handle.createReadStream({ start, end, autoClose: false }),
				response,
			);
		} catch (error) {
			if (response.headersSent || response.destroyed) {
				response.destroy();
				return;
			}
			throw error;
		} finally {
			request.off("aborted", abort);
			response.off("close", abort);
			await rendered?.handle.close();
		}
	};
}
