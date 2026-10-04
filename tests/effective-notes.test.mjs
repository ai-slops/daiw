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

async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'daiw-effective-notes-'));
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
  const rpc = async (request, status = 200) => {
    const response = await fetch(`${base}/api/rpc`, {
      method: 'POST', headers: { 'content-type': 'application/json', 'X-DAIW-Session': token },
      body: JSON.stringify(request),
    });
    const payload = await response.json();
    assert.equal(response.status, status, JSON.stringify(payload));
    return payload;
  };
  return { base, rpc };
}

const cases = [
  { name: 'duplicate source IDs', operations: (state) => [{ type: 'addNotes', notes: [state.definitions[0].notes[0]] }] },
  { name: 'overflow from finite transposes', operations: () => [{ type: 'transpose', semitones: Number.MAX_VALUE }, { type: 'transpose', semitones: Number.MAX_VALUE }] },
];

for (const scenario of cases) {
  test(`HTTP rejects ${scenario.name} without normalizing or replacing canonical state`, async (t) => {
    const { base, rpc } = await fixture(t);
    let { state } = await rpc({ method: 'project.create', params: { fixture: true } });
    const instanceId = state.instances[0].id;
    ({ state } = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'createModifier', name: scenario.name, operations: scenario.operations(state) } } }));
    const modifierId = state.modifiers.at(-1).id;
    const command = { type: 'applyModifier', instanceId, modifierId };
    const preview = await rpc({ projectId: state.id, method: 'change.dryRun', params: { expectedRevision: state.revision, command } });
    assert.equal(preview.result.executable, true);
    assert.equal(preview.result.noteResolution.ready, false);
    assert.equal(preview.result.diagnostics[0].code, 'InvalidProject');
    assert.deepEqual(preview.state, state);
    ({ state } = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command } }));
    for (const [method, params] of [['project.explain', { instanceId }], ['project.snapshot', {}]]) {
      const failure = await rpc({ projectId: state.id, method, params }, 422);
      assert.equal(failure.error.code, 'InvalidProject');
      assert.equal(failure.state, undefined);
    }
    const rendered = await fetch(`${base}/api/projects/${state.id}/render.wav`);
    assert.equal(rendered.status, 422);
    assert.equal((await rendered.json()).error.code, 'InvalidProject');
    const exported = await (await fetch(`${base}/api/projects/${state.id}/export`)).json();
    assert.deepEqual(exported.content, state);
    const imported = await rpc({ method: 'project.import', params: { project: exported } });
    const reopened = await rpc({ projectId: imported.state.id, method: 'project.snapshot', params: {} }, 422);
    assert.equal(reopened.error.code, 'InvalidProject');
    ({ state } = await rpc({ projectId: state.id, method: 'project.undo', params: { expectedRevision: state.revision } }));
    const recovered = await rpc({ projectId: state.id, method: 'project.explain', params: { instanceId } });
    assert.equal(recovered.result.effectiveNotes.length, 4);
    assert.ok(recovered.result.effectiveNotes.every((note) => Number.isFinite(note.pitch)));
  });
}
