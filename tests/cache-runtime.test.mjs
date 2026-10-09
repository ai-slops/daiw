import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { test } from 'node:test';
import { ProjectService } from '../app/service.mjs';

const counterKeys = ['evictions', 'expandedWork', 'hits', 'misses'];

function assertCacheStats(stats) {
  assert.deepEqual(Object.keys(stats).sort(), ['enabled', 'lastRequest', 'limits', 'resident', 'totals']);
  assert.equal(typeof stats.enabled, 'boolean');
  assert.deepEqual(Object.keys(stats.limits).sort(), ['maxEntries', 'maxNotes', 'maxPathEntries', 'maxRetainedBytes']);
  assert.deepEqual(Object.keys(stats.resident).sort(), ['entries', 'notes', 'pathEntries', 'retainedBytes']);
  assert.deepEqual(Object.keys(stats.totals).sort(), counterKeys);
  assert.deepEqual(Object.keys(stats.lastRequest).sort(), counterKeys);
  for (const group of [stats.limits, stats.resident, stats.totals, stats.lastRequest]) {
    for (const value of Object.values(group)) assert.ok(Number.isSafeInteger(value) && value >= 0);
  }
}

function zeroCounters() {
  return { hits: 0, misses: 0, evictions: 0, expandedWork: 0 };
}

test('cache diagnostics are denied to agents at the host boundary', async (t) => {
  // Given
  const directory = await mkdtemp(join(tmpdir(), 'daiw-cache-auth-'));
  const service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  t.after(async () => {
    await service.close();
    await rm(directory, { recursive: true });
  });
  const created = await service.execute({ method: 'project.create', params: { fixture: true } });
  assert.equal(created.ok, true, JSON.stringify(created));

  // When / Then
  await assert.rejects(
    service.execute({ projectId: created.state.id, method: 'project.cacheStats', params: {} }, 'agent'),
    { code: 'PermissionDenied', status: 403 },
  );
});

test('bounded composition cache reuses work, invalidates dependencies, and remains process-local', async (t) => {
  // Given a nested fixture and a cold real worker.
  const directory = await mkdtemp(join(tmpdir(), 'daiw-cache-runtime-'));
  let service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  t.after(async () => {
    await service.close();
    await rm(directory, { recursive: true });
  });
  const created = await service.execute({ method: 'project.create', params: { fixture: true } });
  assert.equal(created.ok, true, JSON.stringify(created));
  let state = created.state;
  const rootInstance = state.instances[0];
  assert.ok(rootInstance);
  const root = state.definitions.find((definition) => definition.id === rootInstance.definitionId);
  assert.ok(root);
  const child = state.definitions.find((definition) => definition.id !== root.id && definition.notes.length > 1);
  assert.ok(child);
  const nested = await service.execute({
    projectId: state.id,
    method: 'change.apply',
    params: {
      expectedRevision: state.revision,
      command: {
        type: 'addDefinitionChild', parentDefinitionId: root.id, definitionId: child.id,
        start: 0, length: Math.min(root.length, child.length),
      },
    },
  });
  assert.equal(nested.ok, true, JSON.stringify(nested));
  state = nested.state;

  // When the same snapshot is resolved twice with diagnostics between requests.
  const headBeforeStats = structuredClone(service.store.get(state.id));
  const hashBeforeStats = await service.execute({ projectId: state.id, method: 'project.hash', params: {} });
  const historyBeforeStats = await service.execute({ projectId: state.id, method: 'project.history', params: {} });
  const coldSnapshot = await service.execute({ projectId: state.id, method: 'project.snapshot', params: {} });
  assert.equal(coldSnapshot.ok, true, JSON.stringify(coldSnapshot));
  const diagnosticRequest = { projectId: state.id, method: 'project.cacheStats', params: {}, idempotencyKey: 'cache-diagnostics-query' };
  const coldStats = await service.execute(diagnosticRequest);
  const repeatedStats = await service.execute(diagnosticRequest);
  assert.equal(coldStats.ok, true, JSON.stringify(coldStats));
  assertCacheStats(coldStats.result);
  assert.deepEqual(repeatedStats.result, coldStats.result);
  assert.equal(service.store.hasRequest(state.id, diagnosticRequest.idempotencyKey), false);
  assert.deepEqual(service.store.get(state.id), headBeforeStats);
  const hashAfterStats = await service.execute({ projectId: state.id, method: 'project.hash', params: {} });
  const historyAfterStats = await service.execute({ projectId: state.id, method: 'project.history', params: {} });
  assert.deepEqual(hashAfterStats.result, hashBeforeStats.result);
  assert.deepEqual(hashAfterStats.state, hashBeforeStats.state);
  assert.deepEqual(historyAfterStats.result, historyBeforeStats.result);
  assert.deepEqual(historyAfterStats.state, historyBeforeStats.state);
  assert.ok(coldStats.result.lastRequest.misses > 0);
  assert.ok(coldStats.result.lastRequest.expandedWork > 0);

  const warmSnapshot = await service.execute({ projectId: state.id, method: 'project.snapshot', params: {} });
  const warmStats = await service.execute({ projectId: state.id, method: 'project.cacheStats', params: {} });
  assert.deepEqual(warmSnapshot.result, coldSnapshot.result);
  assert.ok(warmStats.result.lastRequest.hits > 0);
  assert.equal(warmStats.result.lastRequest.misses, 0);
  assert.equal(warmStats.result.lastRequest.expandedWork, 0);

  // Then project-only edits reuse patterns, while a child edit invalidates only dependent roots.
  const tempo = await service.execute({
    projectId: state.id,
    method: 'change.apply',
    params: { expectedRevision: state.revision, command: { type: 'setTempo', tempo: state.tempo + 7 } },
  });
  assert.equal(tempo.ok, true, JSON.stringify(tempo));
  state = tempo.state;
  const tempoSnapshot = await service.execute({ projectId: state.id, method: 'project.snapshot', params: {} });
  const tempoStats = await service.execute({ projectId: state.id, method: 'project.cacheStats', params: {} });
  assert.equal(tempoSnapshot.result.snapshot.tempo, state.tempo);
  assert.deepEqual(tempoSnapshot.result.snapshot.notes, warmSnapshot.result.snapshot.notes);
  assert.ok(tempoStats.result.lastRequest.hits > 0);
  assert.equal(tempoStats.result.lastRequest.misses, 0);
  assert.equal(tempoStats.result.lastRequest.expandedWork, 0);

  const currentChild = state.definitions.find((definition) => definition.id === child.id);
  assert.ok(currentChild);
  const childEdit = await service.execute({
    projectId: state.id,
    method: 'change.apply',
    params: {
      expectedRevision: state.revision,
      command: {
        type: 'editDefinitionNotes', definitionId: child.id,
        notes: currentChild.notes.map((note) => ({ ...note, pitch: note.pitch + 1 })),
      },
    },
  });
  assert.equal(childEdit.ok, true, JSON.stringify(childEdit));
  state = childEdit.state;
  const invalidatedSnapshot = await service.execute({ projectId: state.id, method: 'project.snapshot', params: {} });
  const invalidatedStats = await service.execute({ projectId: state.id, method: 'project.cacheStats', params: {} });
  assert.notDeepEqual(invalidatedSnapshot.result.snapshot.notes, tempoSnapshot.result.snapshot.notes);
  assert.ok(invalidatedStats.result.lastRequest.hits > 0);
  assert.ok(invalidatedStats.result.lastRequest.misses > 0);
  assert.ok(invalidatedStats.result.lastRequest.expandedWork > 0);

  // And closing the service discards diagnostics without changing reproducible output.
  await service.close();
  service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  const restartedStats = await service.execute({ projectId: state.id, method: 'project.cacheStats', params: {} });
  assert.equal(restartedStats.ok, true, JSON.stringify(restartedStats));
  assertCacheStats(restartedStats.result);
  assert.deepEqual(restartedStats.result.resident, { entries: 0, retainedBytes: 0, notes: 0, pathEntries: 0 });
  assert.deepEqual(restartedStats.result.totals, zeroCounters());
  assert.deepEqual(restartedStats.result.lastRequest, zeroCounters());
  const restartedSnapshot = await service.execute({ projectId: state.id, method: 'project.snapshot', params: {} });
  assert.deepEqual(restartedSnapshot.result, invalidatedSnapshot.result);
});
