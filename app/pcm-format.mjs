import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { open, stat } from 'node:fs/promises';
import { z } from 'zod';

export const PCM_FORMAT = 'daiw-f32le-v1';
export const PCM_FORMAT_VERSION = '1';
export const PCM_HEADER_BYTES = 80;
export const MAX_PCM_PAYLOAD_BYTES = 512 * 1024 * 1024;
export const MAX_PCM_FILE_BYTES = MAX_PCM_PAYLOAD_BYTES + PCM_HEADER_BYTES;

const sha256Schema = z.string().regex(/^[a-f0-9]{64}$/);
export const descriptorSchema = z.object({
  format: z.literal(PCM_FORMAT),
  frames: z.number().int().positive(),
  sampleRate: z.number().int().min(8000).max(192000),
  channels: z.number().int().min(1).max(8),
  byteLength: z.number().int().min(PCM_HEADER_BYTES).max(MAX_PCM_FILE_BYTES),
  sha256: sha256Schema,
  peaks128: z.array(z.number().nonnegative()).length(128),
}).superRefine((value, context) => {
  const payloadLength = value.frames * value.channels * 4;
  if (!Number.isSafeInteger(payloadLength) || value.byteLength !== PCM_HEADER_BYTES + payloadLength) {
    context.addIssue({ code: 'custom', path: ['byteLength'], message: 'PCM byte length does not match frame metadata' });
  }
  if (payloadLength > MAX_PCM_PAYLOAD_BYTES) context.addIssue({ code: 'custom', path: ['byteLength'], message: 'PCM payload exceeds 512 MiB' });
  if (value.frames / value.sampleRate > 180) context.addIssue({ code: 'custom', path: ['frames'], message: 'PCM duration exceeds 180 seconds' });
});

const rendererDescriptorSchema = z.object({
  format: z.literal(PCM_FORMAT),
  frames: z.number().int().positive(),
  sampleRate: z.number().int().min(8000).max(192000),
  channels: z.number().int().min(1).max(8),
  byteLength: z.number().int().min(PCM_HEADER_BYTES).max(MAX_PCM_FILE_BYTES),
  sha256: sha256Schema,
  peaks: z.array(z.number().nonnegative()).length(128),
}).strict();

export function parseDescriptor(value) {
  const wire = rendererDescriptorSchema.parse(value);
  return descriptorSchema.parse({
    format: wire.format,
    frames: wire.frames,
    sampleRate: wire.sampleRate,
    channels: wire.channels,
    byteLength: wire.byteLength,
    sha256: wire.sha256,
    peaks128: wire.peaks,
  });
}

export class InvalidPcmCacheError extends Error {
  constructor(message) {
    super(message);
    this.name = 'InvalidPcmCacheError';
  }
}

export async function hashFile(path, options = undefined) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path, options)) hash.update(chunk);
  return hash.digest('hex');
}

function parseHeader(header) {
  if (header.subarray(0, 8).toString('binary') !== 'DAIWPCM\0') throw new InvalidPcmCacheError('PCM magic is invalid.');
  if (header.readUInt16LE(8) !== 1 || header.readUInt16LE(10) !== PCM_HEADER_BYTES) throw new InvalidPcmCacheError('PCM version is unsupported.');
  if (header.readUInt16LE(18) !== 1) throw new InvalidPcmCacheError('PCM sample format is unsupported.');
  if (header.subarray(68, 80).some((value) => value !== 0)) throw new InvalidPcmCacheError('PCM reserved header bytes are nonzero.');
  const frames = Number(header.readBigUInt64LE(20));
  const payloadLength = Number(header.readBigUInt64LE(28));
  if (!Number.isSafeInteger(frames) || !Number.isSafeInteger(payloadLength)) throw new InvalidPcmCacheError('PCM dimensions are unsafe.');
  return {
    sampleRate: header.readUInt32LE(12),
    channels: header.readUInt16LE(16),
    frames,
    payloadLength,
    payloadSha256: header.subarray(36, 68).toString('hex'),
  };
}

export async function verifyPcmFile(path, rawDescriptor) {
  const descriptor = descriptorSchema.parse(rawDescriptor);
  const details = await stat(path);
  if (details.size !== descriptor.byteLength) throw new InvalidPcmCacheError('PCM file size does not match its descriptor.');
  const handle = await open(path, 'r');
  const header = Buffer.alloc(PCM_HEADER_BYTES);
  try {
    const { bytesRead } = await handle.read(header, 0, PCM_HEADER_BYTES, 0);
    if (bytesRead !== PCM_HEADER_BYTES) throw new InvalidPcmCacheError('PCM header is truncated.');
  } finally {
    await handle.close();
  }
  const parsed = parseHeader(header);
  if (parsed.sampleRate !== descriptor.sampleRate || parsed.channels !== descriptor.channels || parsed.frames !== descriptor.frames) {
    throw new InvalidPcmCacheError('PCM header metadata does not match its descriptor.');
  }
  if (parsed.payloadLength !== descriptor.byteLength - PCM_HEADER_BYTES || parsed.payloadLength !== descriptor.frames * descriptor.channels * 4) {
    throw new InvalidPcmCacheError('PCM payload length is invalid.');
  }
  const [wholeSha256, payloadSha256] = await Promise.all([
    hashFile(path),
    hashFile(path, { start: PCM_HEADER_BYTES }),
  ]);
  if (wholeSha256 !== descriptor.sha256 || payloadSha256 !== parsed.payloadSha256) {
    throw new InvalidPcmCacheError('PCM checksum verification failed.');
  }
  return descriptor;
}
