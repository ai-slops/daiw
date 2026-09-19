import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ProjectService } from '../app/service.mjs';

async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'daiw-service-'));
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  t.after(async () => { await service.close(); await rm(directory, { recursive: true }); });
  const created = await service.execute({ method: 'project.create', params: { name: 'Integration', fixture: true } });
  assert.equal(created.ok, true, JSON.stringify(created));
  return { service, state: created.state };
}

test('dry-run describes an edit without moving the durable project head', async (t) => {
  // Given the shared fixture at a known revision.
  const { service, state } = await fixture(t);
  // When tempo is dry-run through the actual Rust worker.
  const response = await service.execute({ projectId: state.id, method: 'change.dryRun', params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: 105 } } });
  // Then the proposal is executable but no canonical mutation occurs.
  assert.equal(response.ok, true, JSON.stringify(response));
  assert.equal(response.result.executable, true);
  assert.equal(service.store.get(state.id).revision, state.revision);
});

test('an agent cannot apply a main edit even with a current revision', async (t) => {
  // Given an agent holding a valid base revision.
  const { service, state } = await fixture(t);
  // When it requests a human-only command, then authorization blocks it.
  await assert.rejects(service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: 105 } } }, 'agent'), { code: 'PermissionDenied' });
  assert.equal(service.store.get(state.id).revision, state.revision);
});

test('a repeated creation request is replayed rather than creating another project', async (t) => {
  // Given a service with one project and an idempotent creation request.
  const { service } = await fixture(t);
  const request = { method: 'project.create', params: { name: 'Idempotent' }, idempotencyKey: 'same-request' };
  const first = await service.execute(request);
  // When the client retries after losing the first response.
  const second = await service.execute(request);
  // Then both responses refer to the same single new project.
  assert.deepEqual(second, first);
  assert.equal(service.store.list().length, 2);
});

test('proposal acceptance changes only the selected instance and selected hunk', async (t) => {
  // Given two independent hunks scoped to one instance.
  const { service, state } = await fixture(t);
  const instance = state.instances[0];
  assert.ok(instance);
  const proposed = await service.execute({ projectId: state.id, method: 'proposal.create', params: { expectedRevision: state.revision, instanceId: instance.id, request: 'An octave higher, more quietly', operations: [{ type: 'transpose', semitones: 12 }, { type: 'gain', db: -3 }] } }, 'agent');
  assert.equal(proposed.ok, true, JSON.stringify(proposed));
  const proposal = proposed.state.proposals.at(-1);
  assert.ok(proposal);
  assert.deepEqual(proposed.state.instances, state.instances);
  // When a human accepts only the first semantic hunk.
  const accepted = await service.execute({ projectId: state.id, method: 'proposal.accept', params: { expectedRevision: proposed.state.revision, proposalId: proposal.id, hunkIds: [proposal.hunks[0].id] } });
  // Then unrelated instances remain unchanged and the other operation is absent.
  assert.equal(accepted.ok, true, JSON.stringify(accepted));
  assert.deepEqual(accepted.state.instances.filter((item) => item.id !== instance.id), state.instances.filter((item) => item.id !== instance.id));
  const modified = accepted.state.instances.find((item) => item.id === instance.id);
  assert.ok(modified.overrides.some((operation) => operation.type === 'transpose'));
  assert.equal(modified.overrides.some((operation) => operation.type === 'gain'), false);
});

test('stale editing cannot overwrite a successful prior mutation', async (t) => {
  // Given a project whose tempo has changed through the worker.
  const { service, state } = await fixture(t);
  const first = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: 105 } } });
  assert.equal(first.ok, true);
  // When an old tab attempts another edit from the original base.
  const stale = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: 140 } } });
  // Then the old command is rejected and the newer tempo survives.
  assert.equal(stale.ok, false);
  assert.equal(stale.error.code, 'StaleRevision');
  assert.equal(service.store.get(state.id).tempo, 105);
});

test('undo checks the caller revision and redo restores the edited tempo', async (t) => {
  // Given one committed tempo change.
  const { service, state } = await fixture(t);
  const edited = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: 105 } } });
  // When a stale undo is attempted, then the current state is protected.
  const stale = await service.execute({ projectId: state.id, method: 'project.undo', params: { expectedRevision: state.revision } });
  assert.equal(stale.ok, false);
  assert.equal(stale.error.code, 'StaleRevision');
  const undone = await service.execute({ projectId: state.id, method: 'project.undo', params: { expectedRevision: edited.state.revision } });
  assert.equal(undone.ok, true, JSON.stringify(undone));
  assert.equal(undone.state.tempo, state.tempo);
  const redone = await service.execute({ projectId: state.id, method: 'project.redo', params: { expectedRevision: undone.state.revision } });
  assert.equal(redone.ok, true, JSON.stringify(redone));
  assert.equal(redone.state.tempo, 105);
  assert.equal(new Set(redone.state.history.map((entry) => entry.id)).size, redone.state.history.length);
});

test('importing an exported project creates a new identity without overwriting the original', async (t) => {
  // Given a project whose exported canonical state is available.
  const { service, state } = await fixture(t);
  // When its native export is imported into the same workspace.
  const imported = await service.execute({ method: 'project.import', params: { project: state } });
  // Then content remains available under a distinct identity and revision.
  assert.equal(imported.ok, true, JSON.stringify(imported));
  assert.notEqual(imported.state.id, state.id);
  assert.notEqual(imported.state.revision, state.revision);
  assert.deepEqual(imported.state.definitions, state.definitions);
  assert.equal(service.store.list().length, 2);
  assert.deepEqual(service.store.get(state.id), state);
});

test('an accepted hunk cannot be applied twice with a fresh request key', async (t) => {
  // Given a fully accepted transpose proposal.
  const { service, state } = await fixture(t);
  const proposed = await service.execute({ projectId: state.id, method: 'proposal.create', params: { expectedRevision: state.revision, instanceId: state.instances[0].id, request: 'octave up' } });
  const proposal = proposed.state.proposals.at(-1);
  const selected = { proposalId: proposal.id, hunkIds: [proposal.hunks[0].id] };
  const accepted = await service.execute({ projectId: state.id, method: 'proposal.accept', params: { expectedRevision: proposed.state.revision, ...selected } });
  assert.equal(accepted.ok, true);
  // When the same hunk is submitted again as a new command.
  const repeated = await service.execute({ projectId: state.id, method: 'proposal.accept', params: { expectedRevision: accepted.state.revision, ...selected } });
  // Then a second octave cannot silently enter the project.
  assert.equal(repeated.ok, false);
  assert.deepEqual(service.store.get(state.id), accepted.state);
});

test('a rejected proposal cannot later mutate main through acceptance', async (t) => {
  // Given a human-rejected proposal.
  const { service, state } = await fixture(t);
  const proposed = await service.execute({ projectId: state.id, method: 'proposal.create', params: { expectedRevision: state.revision, instanceId: state.instances[0].id, request: 'softer' } });
  const proposal = proposed.state.proposals.at(-1);
  const rejected = await service.execute({ projectId: state.id, method: 'proposal.reject', params: { expectedRevision: proposed.state.revision, proposalId: proposal.id } });
  // When its hunk is submitted for acceptance after rejection.
  const accepted = await service.execute({ projectId: state.id, method: 'proposal.accept', params: { expectedRevision: rejected.state.revision, proposalId: proposal.id, hunkIds: [proposal.hunks[0].id] } });
  // Then rejected review state remains terminal and the head is unchanged.
  assert.equal(accepted.ok, false);
  assert.deepEqual(service.store.get(state.id), rejected.state);
});

test('instance explanation returns committed local notes without changing its shared source', async (t) => {
  // Given one linked instance receiving a local note insertion.
  const { service, state } = await fixture(t);
  const instance = state.instances[0];
  const source = state.definitions.find((definition) => definition.id === instance.definitionId);
  const note = { id: 'local-editor-note', start: 0.25, duration: 0.5, pitch: 70, velocity: 0.75 };
  const edited = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'editNotes', instanceId: instance.id, scope: 'instance', notes: [...source.notes, note] } } });
  assert.equal(edited.ok, true, JSON.stringify(edited));
  // When the editor queries its canonical instance context.
  const explained = await service.execute({ projectId: state.id, method: 'project.explain', params: { instanceId: instance.id } });
  // Then the committed local note is present with source identity and local time.
  assert.deepEqual(explained.result.effectiveNotes.find((item) => item.id === note.id), note);
  assert.equal(explained.result.revision, edited.state.revision);
  assert.deepEqual(explained.result.definition.notes, source.notes);
});

test('empty note identities are rejected at the typed command boundary', async (t) => {
  // Given a valid project and an untrusted note payload with no identity.
  const { service, state } = await fixture(t);
  // When the malformed note is submitted through the public wire command.
  const response = await service.execute({ projectId: state.id, method: 'change.apply', params: { expectedRevision: state.revision, command: { type: 'editNotes', instanceId: state.instances[0].id, scope: 'instance', notes: [{ id: '', start: 0, duration: 1, pitch: 60, velocity: 0.5 }] } } });
  // Then the invalid identity cannot enter canonical storage.
  assert.equal(response.ok, false);
  assert.equal(service.store.get(state.id).revision, state.revision);
});
