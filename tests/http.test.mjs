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
  assert.equal(exported.format, 'daiw-project');
  assert.equal(exported.content.tempo, 110);
  const opened = await rpc({ projectId: state.id, method: 'project.get', params: {} });
  assert.deepEqual(opened.body.state, exported.content);
});

test('local note edits preserve later source changes through HTTP and native export', async (t) => {
  const { base, rpc } = await fixture(t);
  const created = await rpc({ method: 'project.create', params: { fixture: true } });
  const original = created.body.state;
  const instanceId = original.instances[0].id;
  const local = original.definitions[0].notes.map((note, index) => index === 0 ? { ...note, pitch: 49 } : note);
  const edited = await rpc({ projectId: original.id, method: 'change.apply', params: { expectedRevision: original.revision, command: { type: 'editNotes', instanceId, scope: 'instance', notes: local } } });
  assert.equal(edited.status, 200, JSON.stringify(edited));
  const source = original.definitions[0].notes.map((note, index) => index === 1 ? { ...note, pitch: 77 } : note);

  const updated = await rpc({ projectId: original.id, method: 'change.apply', params: { expectedRevision: edited.body.state.revision, command: { type: 'editNotes', instanceId, scope: 'source', notes: source } } });

  assert.equal(updated.status, 200, JSON.stringify(updated));
  const explanation = await rpc({ projectId: original.id, method: 'project.explain', params: { instanceId } });
  assert.equal(explanation.status, 200, JSON.stringify(explanation));
  assert.deepEqual(explanation.body.result.effectiveNotes.map((note) => note.pitch), [49, 77, 68, 66]);
  const exported = await (await fetch(`${base}/api/projects/${original.id}/export`)).json();
  assert.deepEqual(exported.content.instances[0].overrides, [{ type: 'patchNotes', updates: [local[0]], added: [], removed: [] }]);
  const imported = await rpc({ method: 'project.import', params: { project: exported } });
  assert.equal(imported.status, 200, JSON.stringify(imported));
  const reopened = await rpc({ projectId: imported.body.state.id, method: 'project.explain', params: { instanceId } });
  assert.deepEqual(reopened.body.result.effectiveNotes, explanation.body.result.effectiveNotes);
  const removed = await rpc({ projectId: original.id, method: 'change.apply', params: { expectedRevision: updated.body.state.revision, command: { type: 'editNotes', instanceId, scope: 'source', notes: source.slice(1) } } });
  assert.equal(removed.status, 200, JSON.stringify(removed));
  const blocked = await fetch(`${base}/api/projects/${original.id}/render.wav`);
  assert.equal(blocked.status, 422);
  assert.equal((await blocked.json()).error.code, 'RebaseConflict');
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

test('agent dry-run reports target conflicts and shared impact without persisting a change', async (t) => {
  const { base, rpc } = await fixture(t);
  const created = await rpc({ method: 'project.create', params: { fixture: true } });
  const original = created.body.state;
  const instanceId = original.instances[0].id;
  const notes = original.definitions[0].notes.map((note, index) => index === 0 ? { ...note, pitch: 49 } : note);
  const edited = await rpc({ projectId: original.id, method: 'change.apply', params: { expectedRevision: original.revision, command: { type: 'editNotes', instanceId, scope: 'instance', notes } } });
  assert.equal(edited.status, 200, JSON.stringify(edited));
  const state = edited.body.state;

  const response = await fetch(`${base}/api/agent`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: 'Bearer test-agent-secret' },
    body: JSON.stringify({ projectId: state.id, method: 'change.dryRun', params: { expectedRevision: state.revision, command: { type: 'editNotes', instanceId, scope: 'source', notes: state.definitions[0].notes.slice(1) } } }),
  });
  const preview = await response.json();

  assert.equal(response.status, 200, JSON.stringify(preview));
  assert.equal(preview.result.executable, true);
  assert.equal(preview.result.noteResolution.ready, false);
  assert.deepEqual(preview.result.affectedInstances, ['instance-chorus-a', 'instance-chorus-b', 'instance-chorus-c']);
  const report = preview.result.noteResolution.instances[0];
  assert.equal(report.before.status, 'clean');
  assert.equal(report.after.status, 'conflict');
  assert.deepEqual(report.after.target, { kind: 'missing', noteId: 'note-chorus-root' });
  assert.deepEqual(report.after.actions, ['restoreMissingNote', 'editInstanceNotes']);
  assert.deepEqual(preview.result.impact.previewRanges.map((range) => range.after), [{ start: 24, end: 32 }, { start: 56, end: 64 }, { start: 120, end: 128 }]);
  assert.deepEqual(preview.state, state);
  const exported = await (await fetch(`${base}/api/projects/${state.id}/export`)).json();
  assert.deepEqual(exported.content, state);
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

test('slot bindings persist and require human revision-bound rebase through HTTP', async (t) => {
  const { base, rpc } = await fixture(t);
  const created = await rpc({ method: 'project.create', params: { fixture: true } });
  let state = created.body.state;
  const instanceId = state.instances[0].id;
  const operations = [{ type: 'replaceSlot', slot: 'ending-fill', notes: [{ ...state.definitions[0].notes[3], id: 'replacement-fill', pitch: 80 }] }];
  const modifier = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'createModifier', name: 'Bound fill', operations } } });
  assert.equal(modifier.status, 200, JSON.stringify(modifier));
  state = modifier.body.state;
  const modifierId = state.modifiers.at(-1).id;
  const attached = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'applyModifier', instanceId, modifierId } } });
  assert.equal(attached.status, 200, JSON.stringify(attached));
  state = attached.body.state;
  assert.equal(state.instances[0].modifierBindings[modifierId].definitionRevision, 0);
  const previousRevision = state.revision;
  const notes = state.definitions[0].notes.map((note, index) => index === 0 ? { ...note, pitch: 62 } : note);
  const changed = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'editNotes', instanceId, scope: 'source', notes } } });
  assert.equal(changed.status, 200, JSON.stringify(changed));
  state = changed.body.state;
  const command = { type: 'rebaseModifier', instanceId, modifierId };
  const blocked = await rpc({ projectId: state.id, method: 'project.snapshot', params: {} });
  assert.equal(blocked.body.error.code, 'RebaseConflict');
  const preview = await rpc({ projectId: state.id, method: 'change.dryRun', params: { expectedRevision: state.revision, command } });
  assert.equal(preview.status, 200, JSON.stringify(preview));
  assert.deepEqual(preview.body.state, state);
  assert.equal(preview.body.result.noteResolution.instances[0].before.binding.reason, 'sourceRevisionChanged');
  assert.equal(preview.body.result.noteResolution.instances[0].after.status, 'clean');
  const stale = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: previousRevision, command } });
  assert.equal(stale.status, 409);
  const agent = await fetch(`${base}/api/agent`, { method: 'POST', headers: { 'content-type': 'application/json', authorization: 'Bearer test-agent-secret' }, body: JSON.stringify({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command } }) });
  assert.equal(agent.status, 403);
  const rebased = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command } });
  assert.equal(rebased.status, 200, JSON.stringify(rebased));
  assert.deepEqual(rebased.body.state.definitions, state.definitions);
  assert.deepEqual(rebased.body.state.modifiers, state.modifiers);
  assert.equal(rebased.body.state.instances[0].modifierBindings[modifierId].definitionRevision, 1);
  const exported = await (await fetch(`${base}/api/projects/${state.id}/export`)).json();
  const imported = await rpc({ method: 'project.import', params: { project: exported } });
  assert.equal(imported.status, 200, JSON.stringify(imported));
  assert.deepEqual(imported.body.state.instances[0].modifierBindings, exported.content.instances[0].modifierBindings);
  const rendered = await rpc({ projectId: imported.body.state.id, method: 'project.snapshot', params: {} });
  assert.equal(rendered.status, 200, JSON.stringify(rendered));
  assert.ok(rendered.body.result.snapshot.notes.some((note) => note.instanceId === instanceId && note.origin.noteId === 'replacement-fill' && note.pitch === 80));
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
