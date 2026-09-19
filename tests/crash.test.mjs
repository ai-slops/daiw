import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { ProjectStore } from '../app/storage.mjs';

test('a killed writer cannot expose a half-committed revision as HEAD', async (t) => {
  // Given a durable original revision and a separate writer process.
  const directory = await mkdtemp(join(tmpdir(), 'daiw-crash-'));
  t.after(() => rm(directory, { recursive: true }));
  const original = { id: 'song', name: 'Before crash', revision: 'committed' };
  const store = new ProjectStore(directory);
  store.commit({ response: { id: 'first', ok: true, state: original, result: {} }, base: null, requestId: 'first', requestHash: 'hash', method: 'project.create', principal: 'human' });
  store.close();
  const child = spawn(process.execPath, ['--input-type=module', '-e', `
    import {DatabaseSync} from 'node:sqlite';
    const db = new DatabaseSync(process.argv[1]);
    db.exec("PRAGMA journal_mode=WAL; BEGIN IMMEDIATE");
    db.prepare('INSERT INTO revisions VALUES (?,?,?,?)').run('uncommitted','song','{}','invalid');
    db.prepare('UPDATE projects SET head=? WHERE id=?').run('uncommitted','song');
    process.stdout.write('transaction-open\\n');
    setInterval(() => {}, 10000);
  `, join(directory, 'state.sqlite')], { stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(() => child.kill('SIGKILL'));
  // When the writer dies after updating HEAD inside a transaction, before commit.
  const ready = await once(child.stdout, 'data', { signal: AbortSignal.timeout(10000) });
  assert.match(ready[0].toString(), /transaction-open/);
  child.kill('SIGKILL');
  await once(child, 'exit');
  // Then SQLite recovery exposes only the last fully committed revision.
  const reopened = new ProjectStore(directory);
  t.after(() => reopened.close());
  assert.deepEqual(reopened.get('song'), original);
  assert.equal(reopened.db.prepare("SELECT COUNT(*) AS n FROM revisions WHERE revision='uncommitted'").get().n, 0);
});
