import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProjectStore } from '../app/storage.mjs';

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'daiw-storage-'));
  const store = new ProjectStore(directory);
  t.after(() => { store.close(); rmSync(directory, { recursive: true }); });
  return store;
}
const initial = { id: 'song', name: 'Shared chorus', revision: 'r1' };
function commit(store, state, base = null, method = 'change.apply') {
  store.commit({ response: { id: 'response', ok: true, state, result: {} }, base, requestId: state.revision, requestHash: `hash-${state.revision}`, method, principal: 'human' });
}

test('a committed snapshot survives closing and reopening its SQLite database', (t) => {
  // Given a new project store.
  const directory = mkdtempSync(join(tmpdir(), 'daiw-reopen-'));
  t.after(() => rmSync(directory, { recursive: true }));
  const first = new ProjectStore(directory);
  commit(first, initial);
  first.close();
  // When the store is reopened after a completed transaction.
  const second = new ProjectStore(directory);
  t.after(() => second.close());
  // Then the canonical snapshot is identical.
  assert.deepEqual(second.get('song'), initial);
});

test('stale compare-and-swap leaves the head and journal unchanged', (t) => {
  // Given a project already advanced to r2.
  const store = fixture(t);
  commit(store, initial);
  const next = { ...initial, revision: 'r2', name: 'Next' };
  commit(store, next, initial);
  // When another command attempts to write from r1.
  assert.throws(() => commit(store, { ...initial, revision: 'r3' }, initial), { code: 'StaleRevision' });
  // Then no partial transaction was persisted.
  assert.deepEqual(store.get('song'), next);
  assert.equal(store.db.prepare('SELECT COUNT(*) AS n FROM journal').get().n, 2);
});

test('idempotency keys replay identical requests and reject different payloads', (t) => {
  // Given a recorded command.
  const store = fixture(t);
  commit(store, initial);
  // When its key is queried with the same or different request hash.
  const replay = store.replay('song', 'r1', 'hash-r1');
  // Then the stored response is returned only for the identical command.
  assert.deepEqual(replay.state, initial);
  assert.throws(() => store.replay('song', 'r1', 'changed'), { code: 'IdempotencyConflict' });
});

test('undo and redo pointers survive ordinary canonical commits', (t) => {
  // Given two committed states.
  const store = fixture(t);
  commit(store, initial);
  const next = { ...initial, revision: 'r2', name: 'Next' };
  commit(store, next, initial);
  // When an undo command commits a fresh revision of the prior content.
  const undone = { ...store.historyTarget('song', 'undo'), revision: 'r3' };
  commit(store, undone, next, 'project.undo');
  // Then redo resolves to the previous head rather than the undo revision.
  assert.deepEqual(store.historyTarget('song', 'redo'), next);
});

test('corrupt canonical payloads fail loudly instead of loading defaults', (t) => {
  // Given an immutable revision whose stored bytes were corrupted externally.
  const store = fixture(t);
  commit(store, initial);
  store.db.prepare('UPDATE revisions SET payload=? WHERE revision=?').run('{}', 'r1');
  // When the project is opened, then integrity verification rejects it.
  assert.throws(() => store.get('song'), { code: 'CorruptProject' });
});
