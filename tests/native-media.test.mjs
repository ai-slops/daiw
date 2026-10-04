import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ProjectService } from '../app/service.mjs';
import { MediaStore } from '../app/media.mjs';

test('imported asset references cannot silently reuse a different local source', async (t) => {
  const directory = await mkdtemp(join(tmpdir(), 'daiw-native-media-'));
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const media = new MediaStore({ directory, service, binary: resolve('target/debug/daiw-render') });
  t.after(async () => { await service.close(); await rm(directory, { recursive: true }); });
  await media.initialize();
  let { state } = await service.execute({ method: 'project.create', params: { fixture: false } });
  ({ state } = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'addTrack', name: 'Audio', role: 'audio', color: '#fff', instrument: 'sample' } } }));
  const frames = 64;
  const bytes = Buffer.alloc(44 + frames * 2);
  bytes.write('RIFF', 0); bytes.writeUInt32LE(bytes.length - 8, 4); bytes.write('WAVEfmt ', 8);
  bytes.writeUInt32LE(16, 16); bytes.writeUInt16LE(1, 20); bytes.writeUInt16LE(1, 22);
  bytes.writeUInt32LE(48000, 24); bytes.writeUInt32LE(96000, 28); bytes.writeUInt16LE(2, 32); bytes.writeUInt16LE(16, 34);
  bytes.write('data', 36); bytes.writeUInt32LE(frames * 2, 40);
  for (let frame = 0; frame < frames; frame++) bytes.writeInt16LE(8000, 44 + frame * 2);
  const form = new FormData();
  form.set('file', new Blob([bytes], { type: 'audio/wav' }), 'source.wav');
  form.set('trackId', state.tracks[0].id); form.set('start', '0'); form.set('expectedRevision', state.revision);
  ({ state } = await media.importAudio(state.id, form));
  assert.ok((await media.render(state.id)).wav.length > 44);
  for (const mismatch of [{ hash: '0'.repeat(64) }, { frames: state.assets[0].frames + 1 }]) {
    const foreign = structuredClone(state);
    Object.assign(foreign.assets[0], mismatch);
    const imported = await service.execute({ method: 'project.import', params: { project: foreign } });
    assert.equal(imported.ok, true, JSON.stringify(imported));
    await assert.rejects(media.render(imported.state.id), { code: 'CorruptAsset' });
    await assert.rejects(media.waveform(imported.state.id, foreign.assets[0].id), { code: 'CorruptAsset' });
  }
});
