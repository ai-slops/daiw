import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { CoreWorker, runBinary } from '../app/worker.mjs';
import { containedPath } from '../app/paths.mjs';

test('a worker exiting during a large write rejects without terminating Node', { skip: process.platform === 'win32' }, async (t) => {
  // Given a subprocess which exits without reading stdin.
  const worker = new CoreWorker('/bin/true');
  t.after(() => worker.close());
  // When the pipe write exceeds its buffer, then both attempts fail safely.
  await assert.rejects(worker.request({ padding: 'x'.repeat(1024 * 1024) }), { code: 'CoreUnavailable' });
  await assert.rejects(worker.request({ padding: 'x'.repeat(1024 * 1024) }), { code: 'CoreUnavailable' });
});

test('malformed worker output is a protocol failure rather than invalid client input', { skip: process.platform === 'win32' }, async (t) => {
  // Given a process echoing input instead of implementing the worker protocol.
  const worker = new CoreWorker('/bin/cat');
  t.after(() => worker.close());
  // When it returns a structurally invalid response, then the host reports 502.
  await assert.rejects(worker.request({ state: null }), { code: 'ProtocolError', status: 502 });
});

test('binary resource exit status is opt-in and does not change general subprocess errors', async () => {
  const command = { binary: process.execPath, args: ['-e', 'process.exit(2)'], input: undefined, limit: 1024 };
  await assert.rejects(runBinary({ ...command, resourceLimitExitCode: 2 }), { code: 'ResourceLimit', status: 413 });
  await assert.rejects(runBinary(command), { code: 'AudioError', status: 422 });
});

for (const [platform, root, expected] of [[path.posix, '/app/dist', '/app/dist/index.html'], [path.win32, 'C:\\app\\dist', 'C:\\app\\dist\\index.html']]) {
  test(`static containment accepts in-root files and rejects escapes on ${platform.sep === '/' ? 'POSIX' : 'Windows'}`, () => {
    // Given a platform-specific static root, when resolving a requested path.
    assert.equal(containedPath(root, '/index.html', platform), expected);
    // Then normal files are accessible and traversal remains blocked.
    assert.throws(() => containedPath(root, '/../private.txt', platform), { code: 'PermissionDenied' });
  });
}
