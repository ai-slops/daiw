import { createHash } from "node:crypto";
import { z } from "zod";

export const MAX_WAV_FILE_BYTES = 34_560_044;
export const WAV_FORMAT_VERSION = "daiw-wav-pcm16-v1";

export class InvalidWavCacheError extends Error {
	constructor(message) {
		super(message);
		this.name = "InvalidWavCacheError";
	}
}

export const renderDescriptorSchema = z
	.object({
		format: z.literal(WAV_FORMAT_VERSION),
		sampleRate: z.literal(48_000),
		channels: z.literal(2),
		frames: z.number().int().min(0).max(8_640_000),
		byteLength: z.number().int().min(44).max(MAX_WAV_FILE_BYTES),
		sha256: z.string().regex(/^[a-f0-9]{64}$/),
	})
	.strict()
	.superRefine((descriptor, context) => {
		if (descriptor.byteLength !== 44 + descriptor.frames * 4) {
			context.addIssue({
				code: "custom",
				path: ["byteLength"],
				message: "WAV length does not match its frame count",
			});
		}
	});

export function parseRenderDescriptor(value) {
	return renderDescriptorSchema.parse(value);
}

function invalid(message) {
	throw new InvalidWavCacheError(message);
}

export async function verifyWavFile(handle, descriptor) {
	const file = await handle.stat();
	if (!file.isFile() || file.size !== descriptor.byteLength)
		invalid("WAV file length is invalid.");
	const header = Buffer.alloc(44);
	const { bytesRead } = await handle.read(header, 0, header.length, 0);
	if (bytesRead !== header.length) invalid("WAV header is truncated.");
	if (
		header.toString("ascii", 0, 4) !== "RIFF" ||
		header.readUInt32LE(4) !== descriptor.byteLength - 8 ||
		header.toString("ascii", 8, 16) !== "WAVEfmt "
	)
		invalid("WAV RIFF header is invalid.");
	if (
		header.readUInt32LE(16) !== 16 ||
		header.readUInt16LE(20) !== 1 ||
		header.readUInt16LE(22) !== 2
	)
		invalid("WAV PCM format is invalid.");
	if (
		header.readUInt32LE(24) !== 48_000 ||
		header.readUInt32LE(28) !== 192_000 ||
		header.readUInt16LE(32) !== 4 ||
		header.readUInt16LE(34) !== 16
	)
		invalid("WAV sample format is invalid.");
	if (
		header.toString("ascii", 36, 40) !== "data" ||
		header.readUInt32LE(40) !== descriptor.frames * 4
	)
		invalid("WAV data header is invalid.");
	const hash = createHash("sha256");
	let scanned = 0;
	let pendingByte;
	let sampleCount = 0;
	let squareSum = 0;
	let pcmPeak = 0;
	for await (const chunk of handle.createReadStream({
		start: 0,
		autoClose: false,
	})) {
		hash.update(chunk);
		const pcm = chunk.subarray(Math.max(0, 44 - scanned));
		let offset = 0;
		if (pendingByte !== undefined && pcm.length > 0) {
			const unsigned = pendingByte | (pcm[0] << 8);
			const sample =
				(unsigned >= 0x8000 ? unsigned - 0x10000 : unsigned) / 32768;
			squareSum += sample * sample;
			pcmPeak = Math.max(pcmPeak, Math.abs(sample));
			sampleCount += 1;
			pendingByte = undefined;
			offset = 1;
		}
		for (; offset + 1 < pcm.length; offset += 2) {
			const sample = pcm.readInt16LE(offset) / 32768;
			squareSum += sample * sample;
			pcmPeak = Math.max(pcmPeak, Math.abs(sample));
			sampleCount += 1;
		}
		if (offset < pcm.length) pendingByte = pcm[offset];
		scanned += chunk.length;
	}
	if (hash.digest("hex") !== descriptor.sha256)
		invalid("WAV integrity hash is invalid.");
	if (
		pendingByte !== undefined ||
		sampleCount !== descriptor.frames * descriptor.channels
	)
		invalid("WAV PCM payload is invalid.");
	return {
		pcmRms: sampleCount === 0 ? 0 : Math.sqrt(squareSum / sampleCount),
		pcmPeak,
	};
}
