import { createHash, randomUUID } from 'node:crypto';
import { mkdir, open, readFile, rename, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { z } from 'zod';
import { runBinary } from './worker.mjs';
import { ServiceError } from './errors.mjs';
import { SourceCache } from './source-cache.mjs';
import { beatAtSeconds, secondsAtBeat, validateMeterMap, validateTempoMap } from '../shared/timing.mjs';

const MAX_RENDER_SOURCES = 64;
const MAX_RENDER_SOURCE_BYTES = 1024 * 1024 * 1024;
const MAX_RENDER_WAV = 64 * 1024 * 1024;
const RENDER_PROFILE = 'daiw-render-file-backed-v1-48000';
const tempoSchema = z.number().min(20).max(400);
const meterSchema = z.tuple([z.number().int().min(1).max(255), z.number().int().min(1).max(255)]);
const tempoMapSchema = z.object({ points: z.array(z.object({ at: z.number().nonnegative(), bpm: tempoSchema, curve: z.enum(['step', 'linear']) })).min(1).max(1024) }).superRefine((map, context) => {
  try { validateTempoMap(map); } catch (error) {
    if (!(error instanceof RangeError)) throw error;
    context.addIssue({ code: 'custom', message: error.message });
  }
});
const meterMapSchema = z.object({ points: z.array(z.object({ at: z.number().nonnegative(), numerator: z.number().int().min(1).max(255), denominator: z.number().int().min(1).max(255) })).min(1).max(1024) }).superRefine((map, context) => {
  try { validateMeterMap(map); } catch (error) {
    if (!(error instanceof RangeError)) throw error;
    context.addIssue({ code: 'custom', message: error.message });
  }
});
const snapshotSchema = z.looseObject({ revision: z.string(), tempo: tempoSchema, meter: meterSchema.default([4, 4]), tempoMap: tempoMapSchema.optional(), meterMap: meterMapSchema.optional(), length: z.number().nonnegative(), regions: z.array(z.looseObject({ assetId: z.string() })).default([]) }).superRefine((snapshot, context) => {
  if (snapshot.tempoMap?.points[0]?.bpm !== undefined && snapshot.tempoMap.points[0].bpm !== snapshot.tempo) context.addIssue({ code: 'custom', path: ['tempoMap'], message: 'tempo must equal the first tempo-map point' });
  const firstMeter = snapshot.meterMap?.points[0];
  if (firstMeter !== undefined && (firstMeter.numerator !== snapshot.meter[0] || firstMeter.denominator !== snapshot.meter[1])) context.addIssue({ code: 'custom', path: ['meterMap'], message: 'meter must equal the first meter-map point' });
});

function missing(error) {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

export function importDurationBeats(startBeat, sourceSeconds, timing) {
  const duration = beatAtSeconds(secondsAtBeat(startBeat, timing) + sourceSeconds, timing) - startBeat;
  return Math.ceil(duration * 960 - 1e-9) / 960;
}

export function renderDurationSeconds(lengthBeats, timing) {
  return secondsAtBeat(lengthBeats, timing);
}

export class MediaStore {
  #renderQueue = Promise.resolve();
  constructor({ directory, binary, service }) {
    this.directory = resolve(directory);
    this.binary = resolve(binary);
    this.service = service;
  }
  async initialize() {
    await Promise.all(['objects', 'cache/audio', 'cache/render'].map((path) => mkdir(join(this.directory, path), { recursive: true })));
    this.rendererFingerprint = createHash('sha256').update(await readFile(this.binary)).digest('hex');
    this.sources = new SourceCache({ directory: this.directory, binary: this.binary, database: this.service.store.db, rendererFingerprint: this.rendererFingerprint });
    await this.sources.initialize();
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
      if (!missing(error)) throw error;
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
    await this.#contentAddressed(join(this.directory, 'objects', hash), bytes);
    const decoded = await this.sources.build(hash);
    if (decoded.frames / decoded.sampleRate > 180) throw new ServiceError('ResourceLimit', 'Import must be shorter than 3 minutes.', 413);
    const asset = { id: `asset-${hash}`, hash, name: file.name.slice(0, 200), frames: decoded.frames, sampleRate: decoded.sampleRate, channels: decoded.channels };
    this.service.store.db.prepare('INSERT OR IGNORE INTO assets VALUES (?,?,?,?)').run(asset.id, hash, asset.name, JSON.stringify(asset));
    const duration = importDurationBeats(fields.start, decoded.frames / decoded.sampleRate, state.tempoMap ?? state.tempo);
    const regionId = createHash('sha256').update(`${projectId}:${idempotencyKey}`).digest('hex');
    return this.service.execute({ projectId, method: 'change.apply', idempotencyKey, params: { expectedRevision: fields.expectedRevision, command: { type: 'addAudioRegion', asset, region: { id: regionId, trackId: fields.trackId, assetId: asset.id, start: fields.start, duration, gain: 0, sourceOffset: 0 } } } });
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
    if (renderDurationSeconds(snapshot.length, snapshot.tempoMap ?? snapshot.tempo) > 180) throw new ServiceError('ResourceLimit', 'This renderer supports arrangements up to 3 minutes.', 413);
    const assetIds = [...new Set(snapshot.regions.map((region) => region.assetId))].sort();
    if (assetIds.length > MAX_RENDER_SOURCES) throw new ServiceError('ResourceLimit', 'Renders support at most 64 distinct audio sources.', 413);
    const sources = {};
    let totalBytes = 0;
    for (const assetId of assetIds) {
      const reference = response.state.assets.find((asset) => asset.id === assetId);
      if (!reference) throw new ServiceError('MissingAsset', 'The snapshot refers to an unknown project asset.', 422);
      const decoded = await this.sources.get(reference);
      totalBytes += decoded.byteLength;
      if (totalBytes > MAX_RENDER_SOURCE_BYTES) throw new ServiceError('ResourceLimit', 'Decoded render sources exceed 1 GB.', 413);
      sources[assetId] = { cacheId: decoded.cacheId, sha256: decoded.sha256, frames: decoded.frames, sampleRate: decoded.sampleRate, channels: decoded.channels, byteLength: decoded.byteLength };
    }
    const identities = assetIds.map((assetId) => [assetId, sources[assetId].sha256]);
    const key = createHash('sha256').update(this.rendererFingerprint).update(JSON.stringify(identities)).update(JSON.stringify(snapshot)).update(RENDER_PROFILE).digest('hex');
    const path = join(this.directory, 'cache/render', `${key}.wav`);
    let wav;
    try { wav = await readFile(path); } catch (error) {
      if (!missing(error)) throw error;
      wav = await runBinary({ binary: this.binary, args: ['--render-file-backed', '--source-root', this.sources.cacheRoot], input: JSON.stringify({ snapshot, sources }), limit: MAX_RENDER_WAV, resourceLimitExitCode: 2 });
      await this.#atomic(path, wav);
    }
    this.service.store.db.prepare('INSERT OR IGNORE INTO renders VALUES (?,?,?,?,?)').run(key, projectId, snapshot.revision, `${RENDER_PROFILE}@${this.rendererFingerprint}`, new Date().toISOString());
    return { wav, revision: snapshot.revision, key };
  }
  async waveform(projectId, assetId) {
    const state = this.service.store.get(projectId);
    const reference = state.assets.find((asset) => asset.id === assetId);
    if (!reference) throw new ServiceError('MissingAsset', 'This source is not part of the project.', 404);
    const decoded = await this.sources.get(reference);
    return { peaks: decoded.peaks128, frames: decoded.frames, sampleRate: decoded.sampleRate, channels: decoded.channels };
  }
}
