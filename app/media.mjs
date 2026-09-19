import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, open, stat } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { z } from 'zod';
import { runBinary } from './worker.mjs';
import { ServiceError } from './errors.mjs';

const MAX_PCM_SAMPLES = 4_000_000;
const MAX_RENDER_SAMPLES = 8_000_000;
const MAX_PCM_JSON = 64 * 1024 * 1024;
const decodedSchema = z.object({ sampleRate: z.number().int().min(8000).max(192000), channels: z.number().int().min(1).max(8), frames: z.number().int().positive().max(MAX_PCM_SAMPLES), samples: z.array(z.number().finite()).max(MAX_PCM_SAMPLES) });
const snapshotSchema = z.object({ revision: z.string(), tempo: z.number().positive(), length: z.number().nonnegative(), regions: z.array(z.object({ assetId: z.string() }).passthrough()).default([]) }).passthrough();

export class MediaStore {
  #renderQueue = Promise.resolve();
  constructor({ directory, binary, service }) {
    this.directory = directory;
    this.binary = binary;
    this.service = service;
  }
  async initialize() {
    await Promise.all(['objects', 'cache/audio', 'cache/render'].map((path) => mkdir(join(this.directory, path), { recursive: true })));
    this.rendererFingerprint = createHash('sha256').update(await readFile(this.binary)).digest('hex');
  }
  async #atomic(path, bytes) {
    await mkdir(dirname(path), { recursive: true });
    const temporary = `${path}.${randomUUID()}.tmp`;
    const handle = await open(temporary, 'wx');
    try { await handle.writeFile(bytes); await handle.sync(); } finally { await handle.close(); }
    await rename(temporary, path);
    if (process.platform !== 'win32') {
      const parent = await open(dirname(path), 'r');
      try { await parent.sync(); } finally { await parent.close(); }
    }
  }
  async #contentAddressed(path, bytes) {
    try {
      await stat(path);
      return;
    } catch (error) {
      if (!(error instanceof Error && 'code' in error && error.code === 'ENOENT')) throw error;
    }
    await this.#atomic(path, bytes);
  }
  async importAudio(projectId, form, idempotencyKey = randomUUID()) {
    z.string().min(1).max(160).parse(idempotencyKey);
    const file = form.get('file');
    if (!(file instanceof File)) throw new ServiceError('InvalidMedia', 'Select a WAV, AIFF, or FLAC audio file.');
    if (file.size > 20 * 1024 * 1024) throw new ServiceError('ResourceLimit', 'Audio imports are limited to 20 MB in this slice.', 413);
    const fields = z.object({ trackId: z.string().min(1), start: z.coerce.number().min(0).max(100000), expectedRevision: z.string().min(1) }).parse({ trackId: form.get('trackId'), start: form.get('start') ?? 0, expectedRevision: form.get('expectedRevision') });
    const current = this.service.store.get(projectId);
    if (current.revision !== fields.expectedRevision && !this.service.store.hasRequest(projectId, idempotencyKey)) throw new ServiceError('StaleRevision', 'Project changed before import. Retry from the latest revision.', 409);
    const state = this.service.store.getRevision(projectId, fields.expectedRevision);
    const bytes = Buffer.from(await file.arrayBuffer());
    const hash = createHash('sha256').update(bytes).digest('hex');
    const path = join(this.directory, 'objects', hash);
    await this.#contentAddressed(path, bytes);
    const decoded = decodedSchema.parse(JSON.parse((await runBinary(this.binary, ['--decode', path], undefined, MAX_PCM_JSON)).toString('utf8')));
    if (decoded.frames / decoded.sampleRate > 180 || decoded.samples.length !== decoded.frames * decoded.channels) throw new ServiceError('ResourceLimit', 'Import must be valid interleaved audio shorter than 3 minutes.', 413);
    const asset = { id: `asset-${hash}`, hash, name: file.name.slice(0, 200), frames: decoded.frames, sampleRate: decoded.sampleRate, channels: decoded.channels };
    await this.#contentAddressed(join(this.directory, 'cache/audio', `${hash}.${this.rendererFingerprint}.json`), JSON.stringify(decoded));
    this.service.store.db.prepare('INSERT OR IGNORE INTO assets VALUES (?,?,?,?)').run(asset.id, hash, asset.name, JSON.stringify(asset));
    const duration = Math.round(decoded.frames / decoded.sampleRate * state.tempo / 60 * 960) / 960;
    const regionId = createHash('sha256').update(`${projectId}:${idempotencyKey}`).digest('hex');
    return this.service.execute({ projectId, method: 'change.apply', idempotencyKey, params: { expectedRevision: fields.expectedRevision, command: { type: 'addAudioRegion', asset, region: { id: regionId, trackId: fields.trackId, assetId: asset.id, start: fields.start, duration, gain: 0, sourceOffset: 0 } } } });
  }
  async #decoded(assetId) {
    const asset = this.service.store.db.prepare('SELECT hash FROM assets WHERE id=?').get(assetId);
    if (!asset) throw new ServiceError('MissingAsset', 'An audio source is missing from this project.', 422);
    const blob = await readFile(join(this.directory, 'objects', asset.hash)).catch((error) => {
      if (error instanceof Error && 'code' in error && error.code === 'ENOENT') throw new ServiceError('MissingAsset', 'An original audio blob is missing.', 422);
      throw error;
    });
    if (createHash('sha256').update(blob).digest('hex') !== asset.hash) throw new ServiceError('CorruptAsset', 'Audio source integrity verification failed.', 422);
    const cached = join(this.directory, 'cache/audio', `${asset.hash}.${this.rendererFingerprint}.json`);
    try {
      if ((await stat(cached)).size > MAX_PCM_JSON) throw new ServiceError('ResourceLimit', 'Decoded source cache exceeds the memory budget.', 413);
      return decodedSchema.parse(JSON.parse(await readFile(cached, 'utf8')));
    } catch (error) {
      if (!(error instanceof Error && 'code' in error && error.code === 'ENOENT')) throw error;
    }
    const result = decodedSchema.parse(JSON.parse((await runBinary(this.binary, ['--decode', join(this.directory, 'objects', asset.hash)], undefined, MAX_PCM_JSON)).toString('utf8')));
    await mkdir(join(this.directory, 'cache/audio'), { recursive: true });
    await this.#atomic(cached, JSON.stringify(result));
    return result;
  }
  render(projectId, proposalId, expectedRevision) {
    const next = this.#renderQueue.then(() => this.#render(projectId, proposalId, expectedRevision));
    this.#renderQueue = next.then(() => undefined, () => undefined);
    return next;
  }
  async #render(projectId, proposalId, expectedRevision) {
    const response = await this.service.execute({ projectId, method: 'project.snapshot', params: proposalId ? { proposalId } : {} });
    if (!response.ok) throw new ServiceError(response.error.code, response.error.message, 422);
    const result = z.object({ snapshot: snapshotSchema }).safeParse(response.result);
    const snapshot = result.success ? result.data.snapshot : snapshotSchema.parse(response.result);
    if (expectedRevision && snapshot.revision !== expectedRevision) throw new ServiceError('StaleRevision', 'The project changed before rendering. Refresh the audition snapshot.', 409);
    if (snapshot.length / snapshot.tempo * 60 > 180) throw new ServiceError('ResourceLimit', 'This renderer supports arrangements up to 3 minutes.', 413);
    const assets = {};
    const assetIds = new Set(snapshot.regions.map((region) => region.assetId));
    let totalSamples = 0;
    for (const assetId of assetIds) {
      const row = this.service.store.db.prepare('SELECT metadata FROM assets WHERE id=?').get(assetId);
      if (!row) throw new ServiceError('MissingAsset', 'An original audio source is missing.', 422);
      const metadata = z.object({ frames: z.number().int().positive(), channels: z.number().int().min(1).max(8) }).parse(JSON.parse(row.metadata));
      totalSamples += metadata.frames * metadata.channels;
      if (totalSamples > MAX_RENDER_SAMPLES) throw new ServiceError('ResourceLimit', 'Sources exceed the 8-million-sample render memory budget.', 413);
    }
    for (const assetId of assetIds) assets[assetId] = await this.#decoded(assetId);
    const input = { ...snapshot, assets };
    const key = createHash('sha256').update(JSON.stringify(input)).update(this.rendererFingerprint).update('daiw-render-v1-48000').digest('hex');
    const path = join(this.directory, 'cache/render', `${key}.wav`);
    let wav;
    try { wav = await readFile(path); } catch (error) {
      if (!(error instanceof Error && 'code' in error && error.code === 'ENOENT')) throw error;
      wav = await runBinary(this.binary, [], JSON.stringify(input));
      await mkdir(join(this.directory, 'cache/render'), { recursive: true });
      await this.#atomic(path, wav);
    }
    this.service.store.db.prepare('INSERT OR IGNORE INTO renders VALUES (?,?,?,?,?)').run(key, projectId, snapshot.revision, `pcm16-stereo-48000-v1@${this.rendererFingerprint}`, new Date().toISOString());
    return { wav, revision: snapshot.revision, key };
  }
  async waveform(projectId, assetId) {
    const state = this.service.store.get(projectId);
    if (!state.assets.some((asset) => asset.id === assetId)) throw new ServiceError('MissingAsset', 'This source is not part of the project.', 404);
    const decoded = await this.#decoded(assetId);
    const width = 128;
    const peaks = Array.from({ length: width }, (_, bin) => {
      const start = Math.floor(bin * decoded.samples.length / width);
      const end = Math.floor((bin + 1) * decoded.samples.length / width);
      let peak = 0;
      for (let i = start; i < end; i++) peak = Math.max(peak, Math.abs(decoded.samples[i]));
      return peak;
    });
    return { peaks, frames: decoded.frames, sampleRate: decoded.sampleRate, channels: decoded.channels };
  }
}
