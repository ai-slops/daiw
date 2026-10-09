import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { chmod, mkdir, mkdtemp, readFile, readdir, rename, rm, stat, symlink, truncate, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ProjectService } from '../app/service.mjs';
import { MediaStore } from '../app/media.mjs';
import { MAX_PCM_FILE_BYTES, parseDescriptor } from '../app/pcm-format.mjs';

const fakeRenderer = `#!/usr/bin/env node
const { createHash } = await import('node:crypto');
const { appendFile, readFile, writeFile } = await import('node:fs/promises');
const args = process.argv.slice(2);
const log = process.env.DAIW_FAKE_MEDIA_LOG;
if (args[0] === '--decode-file') {
  if (args.length !== 5 || args[1] !== '--input' || args[3] !== '--output') process.exit(31);
  const original = await readFile(args[2]);
  const frames = original.includes(Buffer.from('LONG_SOURCE')) ? 4_000_001 : 64;
  const sampleRate = frames > 64 ? 192000 : 48000;
  const payload = Buffer.alloc(frames * 4);
  for (let frame = 0; frame < frames; frame++) payload.writeFloatLE(frame % 2 ? -0.25 : 0.5, frame * 4);
  const payloadHash = createHash('sha256').update(payload).digest();
  const header = Buffer.alloc(80);
  header.write('DAIWPCM\\0', 0, 'binary'); header.writeUInt16LE(1, 8); header.writeUInt16LE(80, 10);
  header.writeUInt32LE(sampleRate, 12); header.writeUInt16LE(1, 16); header.writeUInt16LE(1, 18);
  header.writeBigUInt64LE(BigInt(frames), 20); header.writeBigUInt64LE(BigInt(payload.length), 28);
  payloadHash.copy(header, 36);
  const pcm = Buffer.concat([header, payload]);
  await writeFile(args[4], pcm);
  await appendFile(log, 'decode\\n');
  process.stdout.write(JSON.stringify({ format: 'daiw-f32le-v1', frames, sampleRate, channels: 1, byteLength: pcm.length, sha256: createHash('sha256').update(pcm).digest('hex'), peaks: Array(128).fill(0.5) }));
} else if (args[0] === '--render-file-backed') {
  if (args.length !== 3 || args[1] !== '--source-root') process.exit(32);
  let input = '';
  for await (const chunk of process.stdin) input += chunk;
  const parsed = JSON.parse(input);
  if (Object.keys(parsed).sort().join(',') !== 'snapshot,sources') process.exit(33);
  for (const [assetId, source] of Object.entries(parsed.sources)) {
    if (!assetId || !/^[a-f0-9]{64}$/.test(source.cacheId) || !/^[a-f0-9]{64}$/.test(source.sha256)) process.exit(34);
    const path = args[2] + '/' + source.cacheId + '.pcm';
    const bytes = await readFile(path);
    if (createHash('sha256').update(bytes).digest('hex') !== source.sha256) process.exit(35);
  }
  await appendFile(log, 'render\\n');
  const wav = Buffer.alloc(48); wav.write('RIFF', 0); wav.writeUInt32LE(40, 4); wav.write('WAVEfmt ', 8);
  wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(48000, 24); wav.writeUInt32LE(96000, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34);
  wav.write('data', 36); wav.writeUInt32LE(4, 40); wav.writeInt16LE(1000, 44); wav.writeInt16LE(-1000, 46);
  process.stdout.write(wav);
} else process.exit(30);
`;

async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'daiw-binary-media-'));
  const binary = join(directory, 'fake-renderer.mjs');
  const log = join(directory, 'media.log');
  await writeFile(binary, fakeRenderer);
  await chmod(binary, 0o755);
  await writeFile(log, '');
  const previousLog = process.env.DAIW_FAKE_MEDIA_LOG;
  process.env.DAIW_FAKE_MEDIA_LOG = log;
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const media = new MediaStore({ directory, service, binary });
  await media.initialize();
  t.after(async () => {
    if (previousLog === undefined) delete process.env.DAIW_FAKE_MEDIA_LOG;
    else process.env.DAIW_FAKE_MEDIA_LOG = previousLog;
    await service.close();
    await rm(directory, { recursive: true });
  });
  let { state } = await service.execute({ method: 'project.create', params: { fixture: false } });
  ({ state } = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'addTrack', name: 'Audio', role: 'audio', color: '#fff', instrument: 'sample' } } }));
  return { directory, binary, log, media, service, state };
}

async function importSource(f, marker = 'SHORT_SOURCE') {
  const form = new FormData();
  form.set('file', new Blob([marker], { type: 'audio/wav' }), 'source.wav');
  form.set('trackId', f.state.tracks[0].id);
  form.set('start', '0');
  form.set('expectedRevision', f.state.revision);
  const result = await f.media.importAudio(f.state.id, form);
  f.state = result.state;
  return result;
}

test('decoder publishes the versioned binary PCM contract and accepts sources above four million frames', async (t) => {
  // Given a decoder that emits a valid file-backed descriptor above the former host cap.
  const f = await fixture(t);
  // When the source is imported.
  await importSource(f, 'LONG_SOURCE');
  // Then metadata remains canonical while the managed cache is binary and separately indexed.
  assert.equal(f.state.assets[0].frames, 4_000_001);
  const row = f.service.store.db.prepare('SELECT * FROM decoded_audio_cache').get();
  const expectedCacheId = createHash('sha256').update(f.state.assets[0].hash).update(f.media.rendererFingerprint).update('1').digest('hex');
  assert.equal(row.cache_id, expectedCacheId);
  assert.match(row.cache_id, /^[a-f0-9]{64}$/);
  assert.match(row.sha256, /^[a-f0-9]{64}$/);
  assert.equal(row.frames, 4_000_001);
  const cachePath = join(f.directory, 'cache/audio', `${row.cache_id}.pcm`);
  const bytes = await readFile(cachePath);
  assert.equal(bytes.subarray(0, 8).toString('binary'), 'DAIWPCM\0');
  assert.equal(bytes.length, row.byte_length);
  assert.deepEqual((await readdir(join(f.directory, 'cache/audio'))), [`${row.cache_id}.pcm`]);
});

test('cache corruption is rebuilt once and preserves the render identity', async (t) => {
  // Given one imported source and its deterministic rendered WAV key.
  const f = await fixture(t);
  await importSource(f);
  const first = await f.media.render(f.state.id);
  const row = f.service.store.db.prepare('SELECT * FROM decoded_audio_cache').get();
  const cachePath = join(f.directory, 'cache/audio', `${row.cache_id}.pcm`);
  const pristine = await readFile(cachePath);
  const corruptions = [
    (bytes) => { bytes[80] ^= 0xff; },
    (bytes) => { bytes[8] ^= 0xff; },
    (bytes) => bytes.subarray(0, bytes.length - 1),
    (bytes) => Buffer.concat([bytes, Buffer.from([0])]),
  ];
  // When same-length payload, header, truncation, and trailing-byte corruption are encountered.
  for (const corrupt of corruptions) {
    const candidate = Buffer.from(pristine);
    const changed = corrupt(candidate) ?? candidate;
    await writeFile(cachePath, changed);
    await rm(join(f.directory, 'cache/render', `${first.key}.wav`));
    const rendered = await f.media.render(f.state.id);
    // Then each cache is regenerated from the verified original and keeps the same render key.
    assert.equal(rendered.key, first.key);
    assert.deepEqual(rendered.wav, first.wav);
    assert.deepEqual(await readFile(cachePath), pristine);
  }
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 5);
});

test('a corrupt original is rejected without using an intact decoded fallback', async (t) => {
  // Given a valid imported source and cache.
  const f = await fixture(t);
  await importSource(f);
  const asset = f.state.assets[0];
  const objectPath = join(f.directory, 'objects', asset.hash);
  const original = await readFile(objectPath);
  original[0] ^= 0xff;
  await writeFile(objectPath, original);
  // When waveform and render need that source, then original integrity wins over cache availability.
  await assert.rejects(f.media.waveform(f.state.id, asset.id), { code: 'CorruptAsset' });
  await assert.rejects(f.media.render(f.state.id), { code: 'CorruptAsset' });
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 1);
});

test('cache deletion after restart rebuilds from the original and waveform uses descriptor peaks', async (t) => {
  // Given an imported source whose cache is deleted before a fresh media-store instance starts.
  const f = await fixture(t);
  await importSource(f);
  const row = f.service.store.db.prepare('SELECT * FROM decoded_audio_cache').get();
  await rm(join(f.directory, 'cache/audio', `${row.cache_id}.pcm`));
  const restarted = new MediaStore({ directory: f.directory, service: f.service, binary: f.binary });
  await restarted.initialize();
  // When waveform is requested, then the source is rebuilt and peaks come from the descriptor.
  const waveform = await restarted.waveform(f.state.id, f.state.assets[0].id);
  assert.deepEqual(waveform.peaks, Array(128).fill(0.5));
  assert.equal((await stat(join(f.directory, 'cache/audio', `${row.cache_id}.pcm`))).size, row.byte_length);
});

test('startup cleans only the source temp directory and the decoded cache budget rejects a build', async (t) => {
  // Given owned source temporaries and decoded cache files consuming the hard budget.
  const f = await fixture(t);
  const ownedTemporary = join(f.directory, 'cache/source-temp', 'abandoned.pcm');
  const unrelatedTemporary = join(f.directory, 'cache/render', 'keep.tmp');
  await writeFile(ownedTemporary, 'partial');
  await writeFile(unrelatedTemporary, 'unrelated');
  await f.media.initialize();
  assert.deepEqual(await readdir(join(f.directory, 'cache/source-temp')), []);
  assert.equal(await readFile(unrelatedTemporary, 'utf8'), 'unrelated');
  const reservationBlocker = join(f.directory, 'cache/audio', 'budget.bin');
  await mkdir(join(f.directory, 'cache/audio'), { recursive: true });
  await writeFile(reservationBlocker, '');
  await truncate(reservationBlocker, 4 * 1024 * 1024 * 1024 - 512 * 1024 * 1024 + 1);
  // When an import needs a conservative 512 MiB build reservation, then it fails before decoding.
  await assert.rejects(importSource(f), { code: 'ResourceLimit', status: 413 });
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 0);
});

test('decoder descriptors accept only peaks and enforce payload, finite, and duration bounds', () => {
  const base = { format: 'daiw-f32le-v1', frames: 16_777_216, sampleRate: 192000, channels: 8, byteLength: MAX_PCM_FILE_BYTES, sha256: '0'.repeat(64), peaks: Array(128).fill(0.5) };
  assert.deepEqual(parseDescriptor(base).peaks128, base.peaks);
  assert.throws(() => parseDescriptor({ ...base, peaks: undefined, peaks128: base.peaks }));
  assert.throws(() => parseDescriptor({ ...base, byteLength: MAX_PCM_FILE_BYTES + 1 }));
  assert.throws(() => parseDescriptor({ ...base, frames: 34_560_001, byteLength: 34_560_001 * 8 * 4 + 80 }));
  assert.throws(() => parseDescriptor({ ...base, sampleRate: Number.NaN }));
});

test('malformed rows rebuild once under concurrency and cache hits honor canonical metadata', async (t) => {
  const f = await fixture(t);
  await importSource(f);
  const asset = f.state.assets[0];
  f.service.store.db.prepare('UPDATE decoded_audio_cache SET peaks128=?').run('{broken');
  const waveforms = await Promise.all([f.media.waveform(f.state.id, asset.id), f.media.waveform(f.state.id, asset.id)]);
  assert.deepEqual(waveforms[0], waveforms[1]);
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 2);
  const mismatched = { ...asset, frames: asset.frames + 1 };
  await assert.rejects(f.media.sources.build(asset.hash, mismatched), { code: 'CorruptAsset' });
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 2);
  f.service.store.db.prepare('UPDATE assets SET metadata=? WHERE id=?').run(JSON.stringify(mismatched), asset.id);
  await assert.rejects(f.media.sources.get(mismatched), { code: 'CorruptAsset' });
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 3);
});

test('managed source paths reject symlink escapes without touching outside files', async (t) => {
  const f = await fixture(t);
  await importSource(f);
  const asset = f.state.assets[0];
  const row = f.service.store.db.prepare('SELECT * FROM decoded_audio_cache').get();
  const cachePath = join(f.directory, 'cache/audio', `${row.cache_id}.pcm`);
  const outsideCache = join(f.directory, 'outside-cache.pcm');
  await rename(cachePath, outsideCache);
  await symlink(outsideCache, cachePath);
  await f.media.waveform(f.state.id, asset.id);
  assert.equal((await stat(outsideCache)).size, row.byte_length);
  const objectPath = join(f.directory, 'objects', asset.hash);
  const outsideObject = join(f.directory, 'outside-object.wav');
  await rename(objectPath, outsideObject);
  await symlink(outsideObject, objectPath);
  await assert.rejects(f.media.waveform(f.state.id, asset.id), { code: 'CorruptAsset' });
  assert.equal(await readFile(outsideObject, 'utf8'), 'SHORT_SOURCE');
});

test('restart removes an unindexed published PCM before reserving its rebuild budget', async (t) => {
  const f = await fixture(t);
  await importSource(f);
  const row = f.service.store.db.prepare('SELECT * FROM decoded_audio_cache').get();
  f.service.store.db.prepare('DELETE FROM decoded_audio_cache').run();
  const filler = join(f.directory, 'cache/audio', 'budget.bin');
  await writeFile(filler, '');
  await truncate(filler, 4 * 1024 * 1024 * 1024 - MAX_PCM_FILE_BYTES);
  const restarted = new MediaStore({ directory: f.directory, service: f.service, binary: f.binary });
  await restarted.initialize();
  const waveform = await restarted.waveform(f.state.id, f.state.assets[0].id);
  assert.equal(waveform.frames, 64);
  assert.equal((await readFile(f.log, 'utf8')).split('decode\n').length - 1, 2);
  assert.equal(f.service.store.db.prepare('SELECT cache_id FROM decoded_audio_cache').get().cache_id, row.cache_id);
});
