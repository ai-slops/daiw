import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ProjectService } from '../app/service.mjs';
import { MediaStore } from '../app/media.mjs';
import { createHandler } from '../app/http.mjs';

async function openHost(directory) {
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const media = new MediaStore({ directory, service, binary: resolve('target/debug/daiw-render') });
  await media.initialize();
  const server = createServer(createHandler({ service, media, staticRoot: resolve('dist') }));
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert.ok(address && typeof address === 'object');
  const base = `http://127.0.0.1:${address.port}`;
  const { token } = await (await fetch(`${base}/api/session`)).json();
  return {
    base,
    async rpc(method, params = {}, projectId) {
      const response = await fetch(`${base}/api/rpc`, {
        method: 'POST', headers: { 'content-type': 'application/json', 'x-daiw-session': token },
        body: JSON.stringify({ method, params, projectId }),
      });
      return { status: response.status, ...await response.json() };
    },
    async close() {
      server.closeAllConnections();
      await new Promise((done) => server.close(done));
      await service.close();
    },
  };
}

test('nested source propagation preserves local occurrence edits through HTTP restart and native exchange', async () => {
  // Given a linked parent definition with one nested source placement.
  const directory = await mkdtemp(join(tmpdir(), 'daiw-nested-'));
  let host = await openHost(directory);
  try {
    const created = await host.rpc('project.create', { fixture: true, name: 'Nested integration' });
    assert.equal(created.ok, true);
    let state = created.state;
    const instance = state.instances[0];
    const parent = state.definitions.find((item) => item.id === instance.definitionId);
    const peer = state.instances.find((item) => item.definitionId === parent.id && item.id !== instance.id);
    const child = state.definitions.find((item) => item.id !== parent.id && item.notes.length > 1);
    assert.ok(peer && child);
    const apply = async (command) => {
      const result = await host.rpc('change.apply', { expectedRevision: state.revision, command }, state.id);
      assert.equal(result.ok, true, JSON.stringify(result));
      state = result.state;
      return result;
    };
    await apply({ type: 'addDefinitionChild', parentDefinitionId: parent.id, definitionId: child.id, start: 0, length: Math.min(parent.length, child.length) });
    const explanation = await host.rpc('project.explain', { instanceId: instance.id }, state.id);
    const nested = explanation.result.compositionNotes.filter((note) => note.origin.definitionPath);
    assert.ok(nested.length > 1);
    const target = nested[0];
    const editedNotes = explanation.result.effectiveNotes.map((note) => note.id === target.id ? { ...note, pitch: note.pitch + 12 } : note);
    await apply({ type: 'editNotes', instanceId: instance.id, scope: 'instance', notes: editedNotes });

    // When the child source changes, its untouched occurrences still propagate.
    await apply({ type: 'editDefinitionNotes', definitionId: child.id, notes: child.notes.map((note) => ({ ...note, pitch: note.pitch + 1 })) });
    const local = await host.rpc('project.explain', { instanceId: instance.id }, state.id);
    const linked = await host.rpc('project.explain', { instanceId: peer.id }, state.id);
    assert.equal(local.result.effectiveNotes.find((note) => note.id === target.id).pitch, target.pitch + 12);
    assert.equal(linked.result.effectiveNotes.find((note) => note.id === target.id).pitch, target.pitch + 1);
    assert.equal(local.result.effectiveNotes.find((note) => note.id === nested[1].id).pitch, nested[1].pitch + 1);
    assert.deepEqual(state.definitions.find((item) => item.id === parent.id).notes, parent.notes);

    // Then export/import and restart preserve identities and playable composition.
    const envelope = await (await fetch(`${host.base}/api/projects/${state.id}/export`)).json();
    assert.ok(envelope.requiredFeatures.includes('nested-definitions-v1'));
    const imported = await host.rpc('project.import', { project: envelope });
    assert.equal(imported.ok, true, JSON.stringify(imported));
    await host.close();
    host = await openHost(directory);
    const reopened = await host.rpc('project.get', {}, imported.state.id);
    assert.deepEqual(reopened.state, imported.state);
    const render = await fetch(`${host.base}/api/projects/${imported.state.id}/render.wav`);
    assert.equal(render.status, 200);
    const wav = Buffer.from(await render.arrayBuffer());
    assert.equal(wav.toString('ascii', 0, 4), 'RIFF');
    assert.ok(wav.subarray(44).some((value) => value !== 0));

    const placement = state.definitions.find((item) => item.id === parent.id).children[0];
    const cycle = await host.rpc('change.apply', { expectedRevision: state.revision, command: { type: 'addDefinitionChild', parentDefinitionId: child.id, definitionId: parent.id, start: 0, length: 1 } }, state.id);
    assert.equal(cycle.ok, false);
    assert.equal((await host.rpc('project.get', {}, state.id)).state.revision, state.revision);
    const preview = await host.rpc('change.dryRun', { expectedRevision: state.revision, command: { type: 'removeDefinitionChild', parentDefinitionId: parent.id, childId: placement.id } }, state.id);
    assert.equal(preview.result.executable, true);
    assert.equal(preview.result.noteResolution.ready, false);
    assert.equal((await host.rpc('project.get', {}, state.id)).state.revision, state.revision);
  } finally {
    await host.close();
    await rm(directory, { recursive: true });
  }
});
