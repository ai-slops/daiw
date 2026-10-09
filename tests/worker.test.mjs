import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { watch } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { once } from 'node:events';
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

test('render infrastructure exit status is separately opt-in', async () => {
  // Given a binary using the render-to-file infrastructure exit contract.
  const command = { binary: process.execPath, args: ['-e', 'process.exit(3)'], input: undefined, limit: 1024 };
  // When only exit three is mapped, then infrastructure failure is distinct from resource exhaustion.
  await assert.rejects(runBinary({ ...command, infrastructureExitCode: 3 }), { code: 'RenderInfrastructure', status: 500 });
  await assert.rejects(runBinary(command), { code: 'AudioError', status: 422 });
});

test('aborting a binary waits for close before rejecting', { skip: process.platform === 'win32' }, async () => {
  // Given a child that records its termination before closing.
  const controller = new AbortController();
  const command = "process.on('SIGTERM',()=>{process.stderr.write('closed');process.exit(0)});setInterval(()=>{},1000)";
  const running = runBinary({ binary: process.execPath, args: ['-e', command], input: undefined, limit: 1024, signal: controller.signal });
  // When cancellation is requested, then rejection occurs only after the owned child closes.
  controller.abort();
  await assert.rejects(running, { code: 'RequestAborted' });
});

test('abort force-kills a SIGTERM-resistant child and leaves subprocess execution healthy', { skip: process.platform === 'win32', timeout: 5000 }, async (t) => {
  // Given a child that signals readiness and does not exit on SIGTERM.
  const directory = await mkdtemp(path.join(tmpdir(), 'daiw-worker-abort-'));
  t.after(() => rm(directory, { recursive: true }));
  const ready = path.join(directory, 'ready');
  const changes = watch(directory);
  t.after(() => changes.close());
  const fixture = "require('node:fs').writeFileSync(process.argv[1],'ready');process.on('SIGTERM',()=>{});setInterval(()=>{},1000)";
  const controller = new AbortController();
  const running = runBinary({ binary: process.execPath, args: ['-e', fixture, ready], input: undefined, signal: controller.signal });
  await once(changes, 'change');
  // When aborted, then escalation closes the child and a subsequent process still succeeds.
  controller.abort();
  await assert.rejects(running, { code: 'RequestAborted' });
  const output = await runBinary({ binary: process.execPath, args: ['-e', "process.stdout.write('healthy')"], input: undefined });
  assert.equal(output.toString(), 'healthy');
});

for (const [platform, root, expected] of [[path.posix, '/app/dist', '/app/dist/index.html'], [path.win32, 'C:\\app\\dist', 'C:\\app\\dist\\index.html']]) {
  test(`static containment accepts in-root files and rejects escapes on ${platform.sep === '/' ? 'POSIX' : 'Windows'}`, () => {
    // Given a platform-specific static root, when resolving a requested path.
    assert.equal(containedPath(root, '/index.html', platform), expected);
    // Then normal files are accessible and traversal remains blocked.
    assert.throws(() => containedPath(root, '/../private.txt', platform), { code: 'PermissionDenied' });
  });
}
