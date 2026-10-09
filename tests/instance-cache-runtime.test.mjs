import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { test } from 'node:test';
import { ProjectService } from '../app/service.mjs';

const limits = {
  maxEntries: 64,
  maxRetainedBytes: 32 * 1024 * 1024,
  maxNotes: 100_000,
  maxPathEntries: 500_000,
};
const zeroCounters = { hits: 0, misses: 0, evictions: 0, expandedWork: 0 };

function assertCacheStats(stats) {
  assert.deepEqual(Object.keys(stats).sort(), ['enabled', 'lastRequest', 'limits', 'resident', 'totals']);
  assert.equal(stats.enabled, true);
  assert.deepEqual(stats.limits, limits);
  assert.deepEqual(Object.keys(stats.resident).sort(), ['entries', 'notes', 'pathEntries', 'retainedBytes']);
  for (const group of [stats.resident, stats.totals, stats.lastRequest]) {
    for (const value of Object.values(group)) assert.ok(Number.isSafeInteger(value) && value >= 0);
  }
}

async function openFixture(t, prefix) {
  const directory = await mkdtemp(join(tmpdir(), prefix));
  let service = new ProjectService({ directory, binary: resolve('target/debug/daiw-core') });
  t.after(async () => {
    await service.close();
    await rm(directory, { recursive: true });
  });
  const created = await service.execute({ method: 'project.create', params: { fixture: true } });
  assert.equal(created.ok, true, JSON.stringify(created));
  let state = created.state;
  const target = state.instances.find((instance) => instance.id === 'instance-chorus-a');
  const peer = state.instances.find((instance) => instance.id === 'instance-chorus-b');
  const parent = state.definitions.find((definition) => definition.id === target?.definitionId);
  const child = state.definitions.find((definition) => definition.id === 'def-drums');
  assert.ok(target && peer && parent && child);
  const apply = async (command) => {
    const response = await service.execute({
      projectId: state.id,
      method: 'change.apply',
      params: { expectedRevision: state.revision, command },
    });
    assert.equal(response.ok, true, JSON.stringify(response));
    state = response.state;
  };
  await apply({
    type: 'addDefinitionChild', parentDefinitionId: parent.id, definitionId: child.id,
    start: 0, length: Math.min(parent.length, child.length),
  });
  await apply({ type: 'createModifier', name: 'Physical transpose', operations: [{ type: 'transpose', semitones: 2 }] });
  const modifier = state.modifiers.at(-1);
  assert.ok(modifier && modifier.operations.length > 0);
  await apply({ type: 'applyModifier', instanceId: target.id, modifierId: modifier.id });
  return {
    directory,
    get service() { return service; },
    set service(next) { service = next; },
    get state() { return state; },
    set state(next) { state = next; },
    apply,
    targetId: target.id,
    peerId: peer.id,
    parentId: parent.id,
    childId: child.id,
  };
}

async function stats(fixture, method, idempotencyKey) {
  const response = await fixture.service.execute({
    projectId: fixture.state.id, method, params: {}, idempotencyKey,
  });
  assert.equal(response.ok, true, JSON.stringify(response));
  assertCacheStats(response.result);
  return response.result;
}

test('both cache diagnostics methods are denied to agents at the host boundary', async (t) => {
  // Given
  const fixture = await openFixture(t, 'daiw-instance-cache-auth-');

  // When / Then
  for (const method of ['project.cacheStats', 'project.instanceCacheStats']) {
    await assert.rejects(
      fixture.service.execute({ projectId: fixture.state.id, method, params: {} }, 'agent'),
      { code: 'PermissionDenied', status: 403 },
    );
  }
});

test('source and instance cache diagnostics stay independent, read-only, and process-local', async (t) => {
  // Given a nested, physically modified fixture and a cold real worker.
  const fixture = await openFixture(t, 'daiw-instance-cache-lifecycle-');
  const headBefore = structuredClone(fixture.service.store.get(fixture.state.id));

  // When the same complete snapshot is resolved cold and warm.
  const coldSnapshot = await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  const coldSource = await stats(fixture, 'project.cacheStats', 'source-diagnostics');
  const coldInstance = await stats(fixture, 'project.instanceCacheStats', 'instance-diagnostics');
  const repeatedSource = await stats(fixture, 'project.cacheStats', 'source-diagnostics');
  const repeatedInstance = await stats(fixture, 'project.instanceCacheStats', 'instance-diagnostics');
  assert.deepEqual(repeatedSource, coldSource);
  assert.deepEqual(repeatedInstance, coldInstance);
  assert.ok(coldSource.lastRequest.misses > 0 && coldSource.lastRequest.expandedWork > 0);
  assert.ok(coldInstance.lastRequest.misses > 0 && coldInstance.lastRequest.expandedWork > 0);
  assert.equal(fixture.service.store.hasRequest(fixture.state.id, 'source-diagnostics'), false);
  assert.equal(fixture.service.store.hasRequest(fixture.state.id, 'instance-diagnostics'), false);
  assert.deepEqual(fixture.service.store.get(fixture.state.id), headBefore);

  const warmSnapshot = await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  const warmSource = await stats(fixture, 'project.cacheStats');
  const warmInstance = await stats(fixture, 'project.instanceCacheStats');
  const sourceAfterInstanceStats = await stats(fixture, 'project.cacheStats');
  assert.deepEqual(warmSnapshot.result, coldSnapshot.result);
  assert.equal(warmSource.lastRequest.hits, 6);
  assert.equal(warmSource.lastRequest.misses, 0);
  assert.equal(warmSource.lastRequest.expandedWork, 0);
  assert.ok(warmInstance.lastRequest.hits > 0);
  assert.equal(warmInstance.lastRequest.misses, 0);
  assert.equal(warmInstance.lastRequest.expandedWork, 0);
  assert.deepEqual(sourceAfterInstanceStats.lastRequest, warmSource.lastRequest);
  assert.deepEqual((await stats(fixture, 'project.instanceCacheStats')).lastRequest, warmInstance.lastRequest);

  // Then restart clears both caches without changing the reproducible snapshot.
  await fixture.service.close();
  fixture.service = new ProjectService({ directory: fixture.directory, binary: resolve('target/debug/daiw-core') });
  for (const method of ['project.cacheStats', 'project.instanceCacheStats']) {
    const restarted = await stats(fixture, method);
    assert.deepEqual(restarted.resident, { entries: 0, retainedBytes: 0, notes: 0, pathEntries: 0 });
    assert.deepEqual(restarted.totals, zeroCounters);
    assert.deepEqual(restarted.lastRequest, zeroCounters);
  }
  const restartedSnapshot = await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  assert.deepEqual(restartedSnapshot.result, coldSnapshot.result);
});

test('placement changes hit while repeated local transposes invalidate only one instance', async (t) => {
  // Given a warm snapshot with peer instances of the same nested source.
  const fixture = await openFixture(t, 'daiw-instance-cache-local-');
  const baseline = await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  const beforeStarts = baseline.result.snapshot.notes.filter((note) => note.instanceId === fixture.targetId).map((note) => note.start);

  // When only placement changes, cached instance notes are reused before placement is applied.
  const target = fixture.state.instances.find((instance) => instance.id === fixture.targetId);
  assert.ok(target);
  await fixture.apply({ type: 'moveInstance', instanceId: fixture.targetId, start: target.start + 1 });
  const moved = await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  const moveStats = await stats(fixture, 'project.instanceCacheStats');
  const movedStarts = moved.result.snapshot.notes.filter((note) => note.instanceId === fixture.targetId).map((note) => note.start);
  assert.deepEqual(movedStarts, beforeStarts.map((start) => start + 1));
  assert.ok(moveStats.lastRequest.hits > 0);
  assert.equal(moveStats.lastRequest.misses, 0);
  assert.equal(moveStats.lastRequest.expandedWork, 0);

  // Then each incremental local transpose misses only the changed instance while peers hit.
  let previous = moved;
  for (const semitones of [1, 2]) {
    const selection = await fixture.service.execute({
      projectId: fixture.state.id,
      method: 'project.queryNotes',
      params: { instanceId: fixture.targetId, scope: 'instance', selector: { type: 'all' }, maxResults: 64 },
    });
    await fixture.apply({
      type: 'transformSelectedNotes', selection: selection.result.selection,
      transform: { type: 'transpose', semitones },
    });
    const changed = await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
    const changedStats = await stats(fixture, 'project.instanceCacheStats');
    const priorTarget = previous.result.snapshot.notes.filter((note) => note.instanceId === fixture.targetId);
    const changedTarget = changed.result.snapshot.notes.filter((note) => note.instanceId === fixture.targetId);
    const priorPeer = previous.result.snapshot.notes.filter((note) => note.instanceId === fixture.peerId);
    const changedPeer = changed.result.snapshot.notes.filter((note) => note.instanceId === fixture.peerId);
    assert.deepEqual(changedTarget.map((note) => note.pitch), priorTarget.map((note) => note.pitch + semitones));
    assert.deepEqual(changedPeer, priorPeer);
    assert.equal(changedStats.lastRequest.misses, 1);
    assert.ok(changedStats.lastRequest.hits >= fixture.state.instances.length - 1);
    assert.ok(changedStats.lastRequest.expandedWork > 0);
    previous = changed;
  }
});

test('a source edit invalidates nested and direct dependents while unrelated instances hit', async (t) => {
  // Given every source and instance result is warm.
  const fixture = await openFixture(t, 'daiw-instance-cache-source-');
  await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  const child = fixture.state.definitions.find((definition) => definition.id === fixture.childId);
  assert.ok(child);
  const dependentInstances = fixture.state.instances.filter((instance) =>
    instance.definitionId === fixture.parentId || instance.definitionId === fixture.childId);
  assert.equal(dependentInstances.length, 4);

  // When a nested source note changes.
  await fixture.apply({
    type: 'editDefinitionNotes', definitionId: child.id,
    notes: child.notes.map((note) => ({ ...note, pitch: note.pitch + 1 })),
  });
  await fixture.service.execute({ projectId: fixture.state.id, method: 'project.snapshot', params: {} });
  const instanceStats = await stats(fixture, 'project.instanceCacheStats');
  const sourceStats = await stats(fixture, 'project.cacheStats');

  // Then three nested chorus uses plus the direct drums use miss; bass and pad hit.
  assert.equal(instanceStats.lastRequest.misses, dependentInstances.length);
  assert.equal(instanceStats.lastRequest.hits, fixture.state.instances.length - dependentInstances.length);
  assert.ok(instanceStats.lastRequest.expandedWork > 0);
  assert.ok(sourceStats.lastRequest.misses > 0);
  assert.ok(sourceStats.lastRequest.hits > 0);
});
