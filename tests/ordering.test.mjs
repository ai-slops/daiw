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
  const directory = await mkdtemp(join(tmpdir(), 'daiw-ordering-'));
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const media = new MediaStore({ directory, service, binary: resolve('target/debug/daiw-render') });
  await media.initialize();
  const server = createServer(createHandler({ service, media, staticRoot: resolve('dist'), agentToken: 'ordering-test' }));
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

test('explicit modifier order is dry-runnable, guarded, effective and exportable over HTTP', async (t) => {
  const { base, rpc } = await fixture(t);
  let { state } = await rpc({ method: 'project.create', params: { fixture: true } });
  const instanceId = state.instances[0].id;
  const apply = async (command) => {
    ({ state } = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command } }));
  };
  await apply({ type: 'createModifier', name: 'Transpose', operations: [{ type: 'transpose', semitones: 12 }] });
  const transpose = state.modifiers.at(-1).id;
  await apply({ type: 'createModifier', name: 'Add', operations: [{ type: 'addNotes', notes: [{ id: 'ordering-note', start: 0, duration: 1, pitch: 60, velocity: 0.8 }] }] });
  const add = state.modifiers.at(-1).id;
  await apply({ type: 'applyModifier', instanceId, modifierId: transpose });
  await apply({ type: 'applyModifier', instanceId, modifierId: add });
  const before = state;
  const empty = { type: 'setModifierOrdering', instanceId, ordering: { edges: [] } };
  const dryRun = await rpc({ projectId: state.id, method: 'change.dryRun', params: { expectedRevision: state.revision, command: empty } });
  assert.equal(dryRun.result.executable, false);
  assert.equal(dryRun.result.diagnostics[0].code, 'ModifierOrderRequired');
  assert.deepEqual(dryRun.state, before);
  const cycle = { ...empty, ordering: { edges: [{ before: add, after: transpose }, { before: transpose, after: add }] } };
  const cyclic = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: cycle } }, 422);
  assert.equal(cyclic.error.code, 'ModifierOrderingCycle');
  const valid = { ...empty, ordering: { edges: [{ before: add, after: transpose }] } };
  const denied = await fetch(`${base}/api/agent`, {
    method: 'POST', headers: { authorization: 'Bearer ordering-test', 'content-type': 'application/json' },
    body: JSON.stringify({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: valid } }),
  });
  assert.equal(denied.status, 403);
  const unchanged = await rpc({ projectId: state.id, method: 'project.get', params: {} });
  assert.deepEqual(unchanged.state, before);
  await apply(valid);
  const stale = await rpc({ projectId: state.id, method: 'change.apply', params: { expectedRevision: before.revision, command: valid } }, 409);
  assert.equal(stale.error.code, 'StaleRevision');
  const explained = await rpc({ projectId: state.id, method: 'project.explain', params: { instanceId } });
  assert.deepEqual(explained.result.modifierPlan.order, [add, transpose]);
  assert.equal(explained.result.modifierPlan.mode, 'explicit');
  assert.equal(explained.result.effectiveNotes.find((note) => note.id === 'ordering-note').pitch, 72);
  assert.deepEqual(state.instances[0].modifiers, [transpose, add]);
  const exported = await (await fetch(`${base}/api/projects/${state.id}/export`)).json();
  const imported = await rpc({ method: 'project.import', params: { project: exported } });
  assert.deepEqual(imported.state.instances[0].modifierOrdering, valid.ordering);
  const reopened = await rpc({ projectId: imported.state.id, method: 'project.explain', params: { instanceId } });
  assert.deepEqual(reopened.result.modifierPlan, explained.result.modifierPlan);
});
