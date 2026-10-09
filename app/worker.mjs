import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { randomUUID } from 'node:crypto';
import { z } from 'zod';
import { ServiceError } from './errors.mjs';

const responseSchema = z.discriminatedUnion('ok', [
  z.object({ id: z.string(), ok: z.literal(true), state: z.looseObject({ id: z.string(), name: z.string(), revision: z.string() }), result: z.unknown() }),
  z.object({ id: z.string(), ok: z.literal(false), error: z.object({ code: z.string(), message: z.string() }) }),
]);

export class CoreWorker {
  #child;
  #pending = new Map();
  constructor(binary) {
    this.binary = binary;
  }
  #start() {
    const child = spawn(this.binary, [], { stdio: ['pipe', 'pipe', 'inherit'] });
    this.#child = child;
    const lines = createInterface({ input: child.stdout });
    lines.on('line', (line) => {
      try {
        const message = responseSchema.parse(JSON.parse(line));
        const pending = this.#pending.get(message.id);
        if (!pending) throw new ServiceError('ProtocolError', 'Unexpected worker response.', 502);
        clearTimeout(pending.timer);
        this.#pending.delete(message.id);
        pending.resolve(message);
      } catch (error) {
        this.#fail(new ServiceError('ProtocolError', 'The core returned an invalid protocol response.', 502), child);
      }
    });
    child.on('error', () => this.#fail(new ServiceError('CoreUnavailable', 'The core process could not be started.', 503), child));
    child.stdin.on('error', () => this.#fail(new ServiceError('CoreUnavailable', 'The core input pipe closed.', 503), child));
    child.on('exit', () => {
      this.#fail(new ServiceError('CoreUnavailable', 'The core process stopped. Retry the operation.', 503), child);
    });
  }
  #fail(error, expected = this.#child) {
    if (expected !== this.#child) return;
    const child = this.#child;
    this.#child = undefined;
    child?.kill();
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.#pending.clear();
  }
  request(input) {
    if (!this.#child) this.#start();
    const child = this.#child;
    const id = randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.#fail(new ServiceError('CoreTimeout', 'The core operation timed out.', 504), child), 30000);
      this.#pending.set(id, { resolve, reject, timer });
      child.stdin.write(`${JSON.stringify({ ...input, id })}\n`, (error) => {
        if (error) this.#fail(new ServiceError('CoreUnavailable', 'The core input pipe closed.', 503), child);
      });
    });
  }
  close() {
    this.#fail(new ServiceError('Shutdown', 'Service is shutting down.', 503));
  }
}

export function runBinary({ binary, args, input, limit = 128 * 1024 * 1024, resourceLimitExitCode = undefined }) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    const chunks = [];
    let bytes = 0;
    let stderr = '';
    let settled = false;
    const finish = (error, output) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) { child.kill(); reject(error); } else resolve(output);
    };
    const timer = setTimeout(() => finish(new ServiceError('RenderTimeout', 'Audio processing exceeded its time limit.', 504)), 120000);
    child.stdout.on('data', (chunk) => {
      bytes += chunk.length;
      if (bytes > limit) finish(new ServiceError('ResourceLimit', 'Audio output exceeds the configured limit.', 413));
      else chunks.push(chunk);
    });
    child.stderr.on('data', (chunk) => { stderr = `${stderr}${chunk}`.slice(-4096); });
    child.on('error', (error) => finish(error));
    child.on('close', (code) => {
      if (code === 0) finish(null, Buffer.concat(chunks));
      else if (code === resourceLimitExitCode) finish(new ServiceError('ResourceLimit', 'Audio processing exceeded a configured resource limit.', 413));
      else finish(new ServiceError('AudioError', stderr.trim() || 'Audio processing failed.', 422));
    });
    child.stdin.on('error', (error) => finish(error));
    child.stdin.end(input);
  });
}
