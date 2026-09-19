import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ProjectService } from '../app/service.mjs';
import { MediaStore } from '../app/media.mjs';
import { createHandler } from '../app/http.mjs';

async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'daiw-http-'));
  await writeFile(join(directory, 'index.html'), '<!doctype html><title>DAIW test</title>');
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const media = new MediaStore({ directory, service, binary: resolve('target/debug/daiw-render') });
  await media.initialize();
  const server = createServer(createHandler({ service, media, staticRoot: directory, agentToken: 'test-agent-secret' }));
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
  const session = await (await fetch(`${base}/api/session`)).json();
  const rpc = async (request) => {
    const response = await fetch(`${base}/api/rpc`, { method: 'POST', headers: { 'content-type': 'application/json', 'X-DAIW-Session': session.token }, body: JSON.stringify(request) });
    return { status: response.status, body: await response.json() };
  };
  return { base, rpc, directory, token: session.token, service };
}

test('the HTTP surface creates, edits, exports, and reopens a project', async (t) => {
  // Given the actual host and Rust worker on an isolated ephemeral port.
  const { base, rpc } = await fixture(t);
  const created = await rpc({ method: 'project.create', params: { name: 'HTTP song' } });
  assert.equal(created.status, 200, JSON.stringify(created));
  const state = created.body.state;
  // When a typed tempo edit is committed through HTTP.
  const edited = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: 110 } } });
  // Then export and subsequent open agree on the durable musical result.
  assert.equal(edited.status, 200, JSON.stringify(edited));
  const exported = await (await fetch(`${base}/api/projects/${state.id}/export`)).json();
  assert.equal(exported.tempo, 110);
  const opened = await rpc({ projectId: state.id, method: 'project.get', params: {} });
  assert.deepEqual(opened.body.state, exported);
});

test('a missing session and an agent main-write attempt are rejected at HTTP', async (t) => {
  // Given an application exposing separate human and agent boundaries.
  const { base } = await fixture(t);
  const body = JSON.stringify({ method: 'project.create', params: {} });
  // When requests omit human credentials or use an agent credential for a human method.
  const human = await fetch(`${base}/api/rpc`, { method: 'POST', body });
  const agent = await fetch(`${base}/api/agent`, { method: 'POST', headers: { authorization: 'Bearer test-agent-secret' }, body });
  // Then neither can create a main project.
  assert.equal(human.status, 403);
  assert.equal(agent.status, 403);
});

test('cross-site token retrieval is blocked and invalid JSON receives a typed error', async (t) => {
  // Given the same-origin session boundary.
  const { base, token } = await fixture(t);
  // When a foreign origin asks for a token or a client submits malformed JSON.
  const foreign = await fetch(`${base}/api/session`, { headers: { origin: 'https://untrusted.example' } });
  const invalid = await fetch(`${base}/api/rpc`, { method: 'POST', headers: { 'X-DAIW-Session': token }, body: '{broken' });
  // Then both fail explicitly, without a successful default response.
  assert.equal(foreign.status, 403);
  assert.equal(invalid.status, 400);
  assert.equal((await invalid.json()).error.code, 'InvalidRequest');
  const rebinding = await fetch(`${base}/api/session`, { headers: { host: 'evil.example', origin: 'http://evil.example' } });
  assert.equal(rebinding.status, 403);
});

test('thrown domain errors preserve their HTTP status and code', async (t) => {
  // Given an HTTP request for a project that does not exist.
  const { base } = await fixture(t);

  // When the export route reaches the domain store.
  const response = await fetch(`${base}/api/projects/missing-project/export`);
  const payload = await response.json();

  // Then the domain Not Found remains a typed 404 rather than a generic 500.
  assert.equal(response.status, 404);
  assert.equal(payload.error.code, 'MissingProject');
});

test('master export is a non-silent WAV bound to the project revision', async (t) => {
  // Given the synthetic shared-chorus fixture.
  const { base, rpc } = await fixture(t);
  const created = await rpc({ method: 'project.create', params: { fixture: true } });
  const state = created.body.state;
  // When the renderer exports through the live HTTP surface.
  const response = await fetch(`${base}/api/projects/${state.id}/render.wav`);
  const bytes = Buffer.from(await response.arrayBuffer());
  // Then the output is real audio with revision provenance.
  assert.equal(response.status, 200, bytes.subarray(0, 200).toString());
  assert.equal(bytes.subarray(0, 4).toString(), 'RIFF');
  assert.equal(bytes.subarray(8, 12).toString(), 'WAVE');
  assert.equal(response.headers.get('x-daiw-revision'), state.revision);
  assert.ok(bytes.subarray(44).some((sample) => sample !== 0));
});

test('an imported source still renders after all rebuildable caches are removed', async (t) => {
  // Given a short, valid PCM WAV and an empty project with a synth track.
  const { base, rpc, directory, token } = await fixture(t);
  const created = await rpc({ method: 'project.create', params: { fixture: false } });
  const tracked = await rpc({ projectId: created.body.state.id, method: 'change.apply', params: { expectedRevision: created.body.state.revision, command: { type: 'addTrack', name: 'Audio', instrument: 'sample', color: '#78a9c2', role: 'audio' } } });
  assert.equal(tracked.status, 200, JSON.stringify(tracked));
  const state = tracked.body.state;
  const frames = 4410;
  const wav = Buffer.alloc(44 + frames * 2);
  wav.write('RIFF', 0); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
  wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(44100, 24); wav.writeUInt32LE(88200, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34);
  wav.write('data', 36); wav.writeUInt32LE(frames * 2, 40);
  for (let i = 0; i < frames; i++) wav.writeInt16LE(Math.round(Math.sin(i * 2 * Math.PI * 440 / 44100) * 4000), 44 + i * 2);
  const form = new FormData();
  form.set('file', new Blob([wav], { type: 'audio/wav' }), 'test-tone.wav');
  form.set('trackId', state.tracks[0].id); form.set('start', '0'); form.set('expectedRevision', state.revision);
  const uploadHeaders = { 'X-DAIW-Session': token, 'X-DAIW-Idempotency': 'source-upload-once' };
  const imported = await fetch(`${base}/api/projects/${state.id}/import-audio`, { method: 'POST', headers: uploadHeaders, body: form });
  const importedBody = await imported.json();
  assert.equal(imported.status, 200, JSON.stringify(importedBody));
  const replayed = await fetch(`${base}/api/projects/${state.id}/import-audio`, { method: 'POST', headers: uploadHeaders, body: form });
  assert.equal(replayed.status, 200);
  assert.deepEqual(await replayed.json(), importedBody);
  form.set('expectedRevision', importedBody.state.revision);
  const duplicateContent = await fetch(`${base}/api/projects/${state.id}/import-audio`, {
    method: 'POST',
    headers: { 'X-DAIW-Session': token, 'X-DAIW-Idempotency': 'same-source-new-placement' },
    body: form,
  });
  assert.equal(duplicateContent.status, 200, await duplicateContent.text());
  const peaks = await (await fetch(`${base}/api/projects/${state.id}/assets/${importedBody.state.assets[0].id}/peaks`)).json();
  assert.equal(peaks.peaks.length, 128);
  assert.ok(peaks.peaks.every((peak) => peak > 0 && peak < 1));
  // When rebuildable caches disappear.
  await rm(join(directory, 'cache'), { recursive: true });
  const rendered = await fetch(`${base}/api/projects/${state.id}/render.wav`);
  // Then original blobs suffice to regenerate audio.
  assert.equal(rendered.status, 200, rendered.status === 200 ? '' : await rendered.text());
  assert.equal(rendered.headers.get('content-type'), 'audio/wav');
  const output = Buffer.from(await rendered.arrayBuffer());
  assert.ok(output.subarray(44).some((sample) => sample !== 0), 'Imported audio must not silently become zero-gain silence.');
});
