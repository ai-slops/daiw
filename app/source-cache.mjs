import { createHash, randomUUID } from 'node:crypto';
import { mkdir, open, readdir, realpath, rename, rm, stat } from 'node:fs/promises';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { z } from 'zod';
import { ServiceError } from './errors.mjs';
import { descriptorSchema, hashFile, InvalidPcmCacheError, MAX_PCM_FILE_BYTES, parseDescriptor, PCM_FORMAT_VERSION, verifyPcmFile } from './pcm-format.mjs';
import { runBinary } from './worker.mjs';

const DESCRIPTOR_STDOUT_LIMIT = 16 * 1024;
const CACHE_BUDGET_BYTES = 4 * 1024 * 1024 * 1024;
const BUILD_RESERVATION_BYTES = MAX_PCM_FILE_BYTES;
const sourceHashSchema = z.string().regex(/^[a-f0-9]{64}$/);
const metadataSchema = z.object({
  hash: sourceHashSchema,
  frames: z.number().int().positive(),
  channels: z.number().int().min(1).max(8),
  sampleRate: z.number().int().min(8000).max(192000),
}).superRefine((value, context) => {
  if (value.frames / value.sampleRate > 180) context.addIssue({ code: 'custom', path: ['frames'], message: 'PCM duration exceeds 180 seconds' });
});

function missing(error) {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

async function directoryBytes(root) {
  let total = 0;
  for (const entry of await readdir(root, { withFileTypes: true }).catch((error) => {
    if (missing(error)) return [];
    throw error;
  })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) total += await directoryBytes(path);
    else if (entry.isFile()) total += (await stat(path)).size;
  }
  return total;
}

export class SourceCache {
  #queue = Promise.resolve();
  constructor({ directory, binary, database, rendererFingerprint }) {
    this.directory = resolve(directory);
    this.binary = resolve(binary);
    this.database = database;
    this.rendererFingerprint = rendererFingerprint;
    this.cacheRoot = resolve(this.directory, 'cache/audio');
    this.tempRoot = resolve(this.directory, 'cache/source-temp');
    this.objectRoot = resolve(this.directory, 'objects');
  }
  async initialize() {
    await rm(this.tempRoot, { recursive: true, force: true });
    await Promise.all([mkdir(this.cacheRoot, { recursive: true }), mkdir(this.tempRoot, { recursive: true }), mkdir(this.objectRoot, { recursive: true })]);
    [this.cacheRoot, this.tempRoot, this.objectRoot] = await Promise.all([realpath(this.cacheRoot), realpath(this.tempRoot), realpath(this.objectRoot)]);
  }
  cacheId(sourceHash) {
    return createHash('sha256').update(sourceHash).update(this.rendererFingerprint).update(PCM_FORMAT_VERSION).digest('hex');
  }
  #managedCachePath(cacheId) {
    const path = resolve(this.cacheRoot, `${cacheId}.pcm`);
    const pathRelative = relative(this.cacheRoot, path);
    if (pathRelative.startsWith(`..${sep}`) || pathRelative === '..' || pathRelative === '' || pathRelative.includes(sep)) {
      throw new ServiceError('CorruptAsset', 'Decoded cache identity is invalid.', 422);
    }
    return path;
  }
  async #realManagedPath(root, path, error) {
    const actual = await realpath(path);
    const pathRelative = relative(root, actual);
    if (pathRelative === '' || pathRelative === '..' || pathRelative.startsWith(`..${sep}`) || resolve(root, pathRelative) !== actual) throw error;
    return actual;
  }
  async #assertRoots() {
    const roots = await Promise.all([realpath(this.cacheRoot), realpath(this.tempRoot), realpath(this.objectRoot)]);
    if (roots[0] !== this.cacheRoot || roots[1] !== this.tempRoot || roots[2] !== this.objectRoot) {
      throw new ServiceError('CorruptAsset', 'Managed audio roots escaped their canonical locations.', 422);
    }
  }
  async #verifiedOriginal(sourceHash) {
    sourceHashSchema.parse(sourceHash);
    const path = resolve(this.objectRoot, sourceHash);
    let actual;
    let managed;
    try {
      await this.#assertRoots();
      managed = await this.#realManagedPath(this.objectRoot, path, new ServiceError('CorruptAsset', 'Original audio path escaped managed storage.', 422));
      actual = await hashFile(managed);
    } catch (error) {
      if (missing(error)) throw new ServiceError('MissingAsset', 'An original audio blob is missing.', 422);
      throw error;
    }
    if (actual !== sourceHash) throw new ServiceError('CorruptAsset', 'Audio source integrity verification failed.', 422);
    return managed;
  }
  #stored(sourceHash) {
    const row = this.database.prepare('SELECT * FROM decoded_audio_cache WHERE source_hash=? AND renderer_fingerprint=? AND format_version=?').get(sourceHash, this.rendererFingerprint, PCM_FORMAT_VERSION);
    if (!row) return undefined;
    return descriptorSchema.parse({
      format: 'daiw-f32le-v1', frames: row.frames, sampleRate: row.sample_rate, channels: row.channels,
      byteLength: row.byte_length, sha256: row.sha256, peaks128: JSON.parse(row.peaks128),
    });
  }
  #canonicalMetadata(sourceHash, expected) {
    const row = this.database.prepare('SELECT metadata FROM assets WHERE hash=? LIMIT 1').get(sourceHash);
    let canonical;
    try {
      canonical = row ? metadataSchema.parse(JSON.parse(row.metadata)) : undefined;
    } catch (error) {
      if (!(error instanceof z.ZodError) && !(error instanceof SyntaxError)) throw error;
      throw new ServiceError('CorruptAsset', 'Canonical audio metadata is invalid.', 422);
    }
    if (!expected) return canonical;
    const parsed = metadataSchema.parse(expected);
    if (canonical && (parsed.frames !== canonical.frames || parsed.sampleRate !== canonical.sampleRate || parsed.channels !== canonical.channels)) {
      throw new ServiceError('CorruptAsset', 'Requested audio metadata differs from the canonical asset.', 422);
    }
    return parsed;
  }
  async #existing(sourceHash, expected) {
    const cacheId = this.cacheId(sourceHash);
    const path = this.#managedCachePath(cacheId);
    try {
      const descriptor = this.#stored(sourceHash);
      if (!descriptor) {
        await this.#assertRoots();
        await this.#realManagedPath(this.cacheRoot, path, new InvalidPcmCacheError('Unindexed PCM path escaped managed storage.'));
        throw new InvalidPcmCacheError('Unindexed PCM cache must be rebuilt.');
      }
      const row = this.database.prepare('SELECT cache_id FROM decoded_audio_cache WHERE source_hash=? AND renderer_fingerprint=? AND format_version=?').get(sourceHash, this.rendererFingerprint, PCM_FORMAT_VERSION);
      if (row.cache_id !== cacheId) throw new InvalidPcmCacheError('PCM cache identity is invalid.');
      if (expected && (descriptor.frames !== expected.frames || descriptor.sampleRate !== expected.sampleRate || descriptor.channels !== expected.channels)) {
        throw new InvalidPcmCacheError('PCM cache metadata differs from the canonical asset.');
      }
      await this.#assertRoots();
      const managed = await this.#realManagedPath(this.cacheRoot, path, new InvalidPcmCacheError('PCM cache path escaped managed storage.'));
      await verifyPcmFile(managed, descriptor);
      return { ...descriptor, cacheId, path };
    } catch (error) {
      if (!(error instanceof InvalidPcmCacheError) && !missing(error) && !(error instanceof z.ZodError) && !(error instanceof SyntaxError)) throw error;
      await rm(path, { force: true });
      this.database.prepare('DELETE FROM decoded_audio_cache WHERE source_hash=? AND renderer_fingerprint=? AND format_version=?').run(sourceHash, this.rendererFingerprint, PCM_FORMAT_VERSION);
      return undefined;
    }
  }
  async get(reference) {
    const asset = this.database.prepare('SELECT hash, metadata FROM assets WHERE id=?').get(reference.id);
    if (!asset) throw new ServiceError('MissingAsset', 'An audio source is missing from this project.', 422);
    const metadata = metadataSchema.parse(JSON.parse(asset.metadata));
    if (asset.hash !== reference.hash || metadata.hash !== reference.hash || metadata.frames !== reference.frames || metadata.channels !== reference.channels || metadata.sampleRate !== reference.sampleRate) {
      throw new ServiceError('CorruptAsset', 'The local audio source does not match this project’s asset fingerprint or metadata.', 422);
    }
    return this.#enqueue(() => this.#resolve(asset.hash, metadata));
  }
  build(sourceHash, expected = undefined) {
    sourceHashSchema.parse(sourceHash);
    return this.#enqueue(() => this.#resolve(sourceHash, expected));
  }
  #enqueue(operation) {
    const next = this.#queue.then(operation);
    this.#queue = next.then(() => undefined, () => undefined);
    return next;
  }
  async #resolve(sourceHash, expected) {
    await Promise.all([mkdir(this.cacheRoot, { recursive: true }), mkdir(this.tempRoot, { recursive: true })]);
    const canonical = this.#canonicalMetadata(sourceHash, expected);
    const original = await this.#verifiedOriginal(sourceHash);
    const existing = await this.#existing(sourceHash, canonical);
    if (existing) return existing;
    return this.#build(sourceHash, canonical, original);
  }
  async #build(sourceHash, expected, original) {
    await this.#assertRoots();
    if (await directoryBytes(this.cacheRoot) + await directoryBytes(this.tempRoot) + BUILD_RESERVATION_BYTES > CACHE_BUDGET_BYTES) {
      throw new ServiceError('ResourceLimit', 'Decoded audio cache capacity is exhausted.', 413);
    }
    const temporary = resolve(this.tempRoot, `${randomUUID()}.pcm`);
    let descriptor;
    try {
      const stdout = await runBinary({ binary: this.binary, args: ['--decode-file', '--input', original, '--output', temporary], input: undefined, limit: DESCRIPTOR_STDOUT_LIMIT, resourceLimitExitCode: 2 });
      descriptor = parseDescriptor(JSON.parse(stdout.toString('utf8')));
      const managedTemporary = await this.#realManagedPath(this.tempRoot, temporary, new ServiceError('CorruptAsset', 'Decoder output escaped managed temporary storage.', 422));
      await verifyPcmFile(managedTemporary, descriptor);
      if (descriptor.byteLength > MAX_PCM_FILE_BYTES) throw new ServiceError('ResourceLimit', 'Decoded source payload exceeds 512 MiB.', 413);
      if (expected && (descriptor.frames !== expected.frames || descriptor.sampleRate !== expected.sampleRate || descriptor.channels !== expected.channels)) {
        throw new ServiceError('CorruptAsset', 'Regenerated audio metadata does not match the canonical asset.', 422);
      }
      const handle = await open(temporary, 'r+');
      try { await handle.sync(); } finally { await handle.close(); }
      const cacheId = this.cacheId(sourceHash);
      const path = this.#managedCachePath(cacheId);
      await this.#assertRoots();
      await rename(temporary, path);
      if (process.platform !== 'win32') {
        const parent = await open(dirname(path), 'r');
        try { await parent.sync(); } finally { await parent.close(); }
      }
      this.database.prepare(`INSERT INTO decoded_audio_cache VALUES (?,?,?,?,?,?,?,?,?,?)
        ON CONFLICT(source_hash,renderer_fingerprint,format_version) DO UPDATE SET cache_id=excluded.cache_id,sha256=excluded.sha256,frames=excluded.frames,sample_rate=excluded.sample_rate,channels=excluded.channels,byte_length=excluded.byte_length,peaks128=excluded.peaks128`)
        .run(sourceHash, this.rendererFingerprint, PCM_FORMAT_VERSION, cacheId, descriptor.sha256, descriptor.frames, descriptor.sampleRate, descriptor.channels, descriptor.byteLength, JSON.stringify(descriptor.peaks128));
      return { ...descriptor, cacheId, path };
    } finally {
      await rm(temporary, { force: true });
    }
  }
}
