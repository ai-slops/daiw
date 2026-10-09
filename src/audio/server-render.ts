import ky from "ky";
import { z } from "zod";

export const CHUNK_FRAMES = 24_000;
export const RENDER_SAMPLE_RATE = 48_000;

const renderInfoSchema = z
	.object({
		format: z.literal("daiw-wav-pcm16-v1"),
		revision: z.string().min(1),
		renderKey: z.string().min(1),
		sampleRate: z.literal(RENDER_SAMPLE_RATE),
		channels: z.literal(2),
		frames: z.number().int().nonnegative(),
		byteLength: z.number().int().nonnegative(),
		dataOffset: z.literal(44),
		bytesPerFrame: z.literal(4),
		sha256: z.string().regex(/^[a-f0-9]{64}$/),
		pcmRms: z.number().nonnegative(),
		pcmPeak: z.number().nonnegative(),
	})
	.strict()
	.superRefine((value, context) => {
		if (
			value.byteLength !==
			value.dataOffset + value.frames * value.bytesPerFrame
		) {
			context.addIssue({
				code: "custom",
				path: ["byteLength"],
				message: "byteLength must describe the PCM payload",
			});
		}
	});

export type RenderInfo = Readonly<z.infer<typeof renderInfoSchema>>;
export type ServerRenderRequest = Readonly<{
	projectId: string;
	expectedRevision: string;
	proposalId?: string;
}>;
export type RenderDescriptor = Readonly<{
	info: RenderInfo;
	wavUrl: string;
	searchParams: URLSearchParams;
}>;
type BufferFactory = Pick<AudioContext, "createBuffer">;
type PcmChunkRequest = Readonly<{
	startFrame: number;
	endFrame: number;
	signal: AbortSignal;
}>;

export class ServerRenderError extends Error {
	public readonly name = "ServerRenderError";
	public constructor(message: string) {
		super(message);
	}
}

export function snapshotRevisionMatches(
	captured: string,
	current: string | null,
): boolean {
	return current === captured;
}

function assertIdentity(headers: Headers, info: RenderInfo): void {
	const encoding = headers.get("Content-Encoding");
	const valid =
		headers.get("ETag") === `"${info.renderKey}"` &&
		headers.get("X-DAIW-Revision") === info.revision &&
		headers.get("X-DAIW-SHA256") === info.sha256 &&
		(encoding === null || encoding === "identity");
	if (!valid)
		throw new ServerRenderError(
			"render identity headers do not match metadata",
		);
}

export async function openServerRender(
	request: ServerRenderRequest,
	signal?: AbortSignal,
): Promise<RenderDescriptor> {
	const root = `/api/projects/${encodeURIComponent(request.projectId)}`;
	const searchParams = new URLSearchParams({
		expectedRevision: request.expectedRevision,
	});
	if (request.proposalId !== undefined)
		searchParams.set("proposalId", request.proposalId);
	const response = await ky.get(`${root}/render-info`, {
		searchParams,
		retry: 0,
		timeout: 15_000,
		...(signal === undefined ? {} : { signal }),
	});
	let info: RenderInfo;
	try {
		info = renderInfoSchema.parse(await response.json());
	} catch (reason) {
		if (reason instanceof z.ZodError)
			throw new ServerRenderError(`invalid render metadata: ${reason.message}`);
		throw reason;
	}
	if (info.revision !== request.expectedRevision) {
		throw new ServerRenderError(
			`render revision mismatch: expected ${request.expectedRevision}, received ${info.revision}`,
		);
	}
	assertIdentity(response.headers, info);
	const wavSearchParams = new URLSearchParams(searchParams);
	wavSearchParams.set("renderKey", info.renderKey);
	return Object.freeze({
		info,
		wavUrl: `${root}/render.wav`,
		searchParams: wavSearchParams,
	});
}

export async function fetchPcmChunk(
	context: BufferFactory,
	descriptor: RenderDescriptor,
	request: PcmChunkRequest,
): Promise<AudioBuffer> {
	const { info } = descriptor;
	const { startFrame, endFrame } = request;
	if (
		!Number.isInteger(startFrame) ||
		!Number.isInteger(endFrame) ||
		startFrame < 0 ||
		endFrame <= startFrame ||
		endFrame > info.frames ||
		endFrame - startFrame > CHUNK_FRAMES
	) {
		throw new ServerRenderError(
			"PCM range must be a non-empty bounded frame interval",
		);
	}
	const firstByte = info.dataOffset + info.bytesPerFrame * startFrame;
	const lastByte = info.dataOffset + info.bytesPerFrame * endFrame - 1;
	const expectedLength = lastByte - firstByte + 1;
	const response = await ky.get(descriptor.wavUrl, {
		searchParams: descriptor.searchParams,
		headers: { Range: `bytes=${firstByte}-${lastByte}` },
		signal: request.signal,
		retry: 0,
		timeout: 15_000,
		throwHttpErrors: false,
	});
	if (
		response.status !== 206 ||
		response.headers.get("Content-Range") !==
			`bytes ${firstByte}-${lastByte}/${info.byteLength}` ||
		response.headers.get("Content-Length") !== String(expectedLength)
	) {
		throw new ServerRenderError(
			"server did not return the exact requested PCM range",
		);
	}
	assertIdentity(response.headers, info);
	const bytes = await response.arrayBuffer();
	if (bytes.byteLength !== expectedLength)
		throw new ServerRenderError("PCM response length changed while reading");
	const frames = endFrame - startFrame;
	const output = context.createBuffer(info.channels, frames, info.sampleRate);
	const input = new DataView(bytes);
	for (let channel = 0; channel < info.channels; channel += 1) {
		const samples = output.getChannelData(channel);
		for (let frame = 0; frame < frames; frame += 1) {
			samples[frame] =
				input.getInt16((frame * info.channels + channel) * 2, true) / 32_768;
		}
	}
	return output;
}
