import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ProjectService } from '../app/service.mjs';
import { MediaStore } from '../app/media.mjs';
import { createHandler } from '../app/http.mjs';

async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'daiw-render-numeric-'));
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const media = new MediaStore({ directory, service, binary: resolve('target/debug/daiw-render') });
  await media.initialize();
  const server = createServer(createHandler({ service, media, staticRoot: resolve('dist') }));
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert.ok(address && typeof address === 'object');
  const base = `http://127.0.0.1:${address.port}`;
  t.after(async () => {
    server.closeAllConnections();
    await new Promise((done) => server.close(done));
    await service.close();
    await rm(directory, { recursive: true });
  });
  const { token } = await (await fetch(`${base}/api/session`)).json();
  const rpc = async (request) => {
    const response = await fetch(`${base}/api/rpc`, { method: 'POST', headers: { 'content-type': 'application/json', 'X-DAIW-Session': token }, body: JSON.stringify(request) });
    const payload = await response.json();
    assert.equal(response.status, 200, JSON.stringify(payload));
    return payload;
  };
  return { base, rpc, service, directory, token };
}

async function rejectedRenderPreservesState(f, state) {
  const snapshot = await f.rpc({ projectId: state.id, method: 'project.snapshot', params: {} });
  assert.equal(snapshot.ok, true);
  for (let retry = 0; retry < 2; retry++) {
    const response = await fetch(`${f.base}/api/projects/${state.id}/render.wav`);
    assert.equal(response.status, 422);
    assert.equal((await response.json()).error.code, 'AudioError');
  }
  assert.deepEqual(await readdir(join(f.directory, 'cache/render')), []);
  assert.equal(f.service.store.db.prepare('SELECT count(*) AS total FROM renders').get().total, 0);
  const exported = await (await fetch(`${f.base}/api/projects/${state.id}/export`)).json();
  assert.deepEqual(exported.content, state);
}

for (const semitones of [Number.MAX_VALUE, -Number.MAX_VALUE]) {
  test(`finite pitch ${semitones > 0 ? 'overflow' : 'underflow'} fails DSP without caching a WAV`, async (t) => {
    const f = await fixture(t);
    let { state } = await f.rpc({ method: 'project.create', params: { fixture: true } });
    const instanceId = state.instances[0].id;
    ({ state } = await f.rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'createModifier', name: 'Unrepresentable frequency', operations: [{ type: 'transpose', semitones }] } } }));
    ({ state } = await f.rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'applyModifier', instanceId, modifierId: state.modifiers.at(-1).id } } }));
    await rejectedRenderPreservesState(f, state);
    ({ state } = await f.rpc({ projectId: state.id, method: 'project.undo', params: { expectedRevision: state.revision } }));
    const recovered = await fetch(`${f.base}/api/projects/${state.id}/render.wav`);
    assert.equal(recovered.status, 200);
    const bytes = Buffer.from(await recovered.arrayBuffer());
    assert.equal(bytes.subarray(0, 4).toString(), 'RIFF');
    assert.ok(bytes.subarray(44).some((value) => value !== 0));
  });
}

test('finite float PCM that overflows region processing fails without a cached render', async (t) => {
  const f = await fixture(t);
  let { state } = await f.rpc({ method: 'project.create', params: { fixture: false } });
  ({ state } = await f.rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'addTrack', name: 'PCM', instrument: 'sample', color: '#fff', role: 'audio' } } }));
  const frames = 64;
  const wav = Buffer.alloc(44 + frames * 4);
  wav.write('RIFF', 0); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
  wav.writeUInt32LE(16, 16); wav.writeUInt16LE(3, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(48000, 24); wav.writeUInt32LE(192000, 28); wav.writeUInt16LE(4, 32); wav.writeUInt16LE(32, 34);
  wav.write('data', 36); wav.writeUInt32LE(frames * 4, 40);
  for (let frame = 0; frame < frames; frame++) wav.writeUInt32LE(0x7f7fffff, 44 + frame * 4);
  const form = new FormData();
  form.set('file', new Blob([wav], { type: 'audio/wav' }), 'finite-hot-float.wav');
  form.set('trackId', state.tracks[0].id); form.set('start', '0'); form.set('expectedRevision', state.revision);
  const upload = await fetch(`${f.base}/api/projects/${state.id}/import-audio`, { method: 'POST', headers: { 'X-DAIW-Session': f.token }, body: form });
  const imported = await upload.json();
  assert.equal(upload.status, 200, JSON.stringify(imported));
  state = imported.state;
  ({ state } = await f.rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'addAudioRegion', asset: state.assets[0], region: { ...state.regions[0], id: 'boosted-region', gain: 24 } } } }));
  await rejectedRenderPreservesState(f, state);
  ({ state } = await f.rpc({ projectId: state.id, method: 'project.undo', params: { expectedRevision: state.revision } }));
  const recovered = await fetch(`${f.base}/api/projects/${state.id}/render.wav`);
  assert.equal(recovered.status, 200);
  const bytes = Buffer.from(await recovered.arrayBuffer());
  assert.ok(bytes.subarray(44).some((value) => value !== 0));
});
