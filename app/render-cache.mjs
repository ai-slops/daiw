import { randomUUID } from 'node:crypto';
import { mkdir, open, readdir, realpath, rename, rm, stat } from 'node:fs/promises';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { z } from 'zod';
import { ServiceError } from './errors.mjs';
import { InvalidWavCacheError, MAX_WAV_FILE_BYTES, parseRenderDescriptor, verifyWavFile } from './wav-format.mjs';

const CACHE_BUDGET_BYTES = 2 * 1024 * 1024 * 1024;
const KEY_PATTERN = /^[a-f0-9]{64}$/;
const FILE_PATTERN = /^([a-f0-9]{64})\.wav$/;
const TEMP_PATTERN = /^[a-f0-9]{64}\.[0-9a-f-]{36}\.tmp$/;

function missing(error) {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

async function directoryBytes(root) {
  let total = 0;
  for (const entry of await readdir(root, { withFileTypes: true })) {
    if (entry.isFile()) total += (await stat(join(root, entry.name))).size;
  }
  return total;
}

export class RenderCache {
  constructor({ directory, database }) {
    this.database = database;
    this.dataRoot = resolve(directory);
    this.cacheRoot = resolve(this.dataRoot, 'cache/render');
    this.tempRoot = resolve(this.dataRoot, 'cache/render-temp');
  }
  async initialize() {
    await mkdir(this.dataRoot, { recursive: true });
    this.dataRoot = await realpath(this.dataRoot);
    this.cacheRoot = resolve(this.dataRoot, 'cache/render');
    this.tempRoot = resolve(this.dataRoot, 'cache/render-temp');
    await Promise.all([mkdir(this.cacheRoot, { recursive: true }), mkdir(this.tempRoot, { recursive: true })]);
    await this.#assertRoots();
    for (const entry of await readdir(this.tempRoot, { withFileTypes: true })) {
      if (entry.isFile() && TEMP_PATTERN.test(entry.name)) await rm(join(this.tempRoot, entry.name), { force: true });
    }
    for (const entry of await readdir(this.cacheRoot, { withFileTypes: true })) {
      const match = entry.isFile() ? FILE_PATTERN.exec(entry.name) : null;
      if (match && !this.database.prepare('SELECT 1 FROM render_cache_files WHERE key=?').get(match[1])) await rm(join(this.cacheRoot, entry.name), { force: true });
    }
  }
  #path(key) {
    if (!KEY_PATTERN.test(key)) throw new ServiceError('CorruptRender', 'Render cache identity is invalid.', 500);
    return resolve(this.cacheRoot, `${key}.wav`);
  }
  async #assertRoots() {
    const roots = await Promise.all([realpath(this.dataRoot), realpath(this.cacheRoot), realpath(this.tempRoot)]);
    if (roots[0] !== this.dataRoot || roots[1] !== resolve(this.dataRoot, 'cache/render') || roots[2] !== resolve(this.dataRoot, 'cache/render-temp')) {
      throw new ServiceError('CorruptRender', 'Managed render roots escaped the canonical data directory.', 500);
    }
  }
  async #realManaged(root, path) {
    const actual = await realpath(path);
    const fromRoot = relative(root, actual);
    if (fromRoot === '' || fromRoot === '..' || fromRoot.startsWith(`..${sep}`) || fromRoot.includes(sep) || resolve(root, fromRoot) !== actual) {
      throw new InvalidWavCacheError('WAV path escaped managed storage.');
    }
    return actual;
  }
  #stored(key) {
    const row = this.database.prepare('SELECT * FROM render_cache_files WHERE key=?').get(key);
    if (!row) return undefined;
    return parseRenderDescriptor({ format: row.format, sampleRate: row.sample_rate, channels: row.channels, frames: row.frames, byteLength: row.byte_length, sha256: row.sha256 });
  }
  #invalidate(key, originalError) {
    let began = false;
    try {
      this.database.exec('BEGIN IMMEDIATE');
      began = true;
      this.database.prepare('DELETE FROM render_cache_files WHERE key=?').run(key);
      this.database.prepare('DELETE FROM renders WHERE key=?').run(key);
      this.database.exec('COMMIT');
    } catch (metadataError) {
      if (began) {
        try { this.database.exec('ROLLBACK'); }
        catch (rollbackError) { originalError.rollbackError = rollbackError; }
      }
      originalError.cause = metadataError;
      throw originalError;
    }
  }
  async open(key) {
    await Promise.all([mkdir(this.cacheRoot, { recursive: true }), mkdir(this.tempRoot, { recursive: true })]);
    const path = this.#path(key);
    let handle;
    try {
      const descriptor = this.#stored(key);
      if (!descriptor) {
        await this.#assertRoots();
        await this.#realManaged(this.cacheRoot, path);
        throw new InvalidWavCacheError('Unindexed WAV cache must be rebuilt.');
      }
      await this.#assertRoots();
      handle = await open(await this.#realManaged(this.cacheRoot, path), 'r');
      await verifyWavFile(handle, descriptor);
      return { handle, descriptor };
    } catch (error) {
      await handle?.close();
      if (!missing(error) && !(error instanceof InvalidWavCacheError) && !(error instanceof z.ZodError) && !(error instanceof SyntaxError)) throw error;
      await rm(path, { force: true });
      this.#invalidate(key, error);
      return undefined;
    }
  }
  async build({ key, projectId, revision, profile, produce }) {
    await Promise.all([mkdir(this.cacheRoot, { recursive: true }), mkdir(this.tempRoot, { recursive: true })]);
    await this.#assertRoots();
    const used = await directoryBytes(this.cacheRoot) + await directoryBytes(this.tempRoot);
    if (used + MAX_WAV_FILE_BYTES > CACHE_BUDGET_BYTES) throw new ServiceError('ResourceLimit', 'Render cache capacity is exhausted.', 413);
    const temporary = resolve(this.tempRoot, `${key}.${randomUUID()}.tmp`);
    const path = this.#path(key);
    let handle;
    let published = false;
    let ownsTemporary = true;
    try {
      const descriptor = await produce(temporary);
      const temporaryHandle = await open(await this.#realManaged(this.tempRoot, temporary), 'r+');
      try { await verifyWavFile(temporaryHandle, descriptor); await temporaryHandle.sync(); } finally { await temporaryHandle.close(); }
      await this.#assertRoots();
      await rename(temporary, path);
      ownsTemporary = false;
      published = true;
      if (process.platform !== 'win32') {
        const parent = await open(dirname(path), 'r');
        try { await parent.sync(); } finally { await parent.close(); }
      }
      handle = await open(await this.#realManaged(this.cacheRoot, path), 'r');
      await verifyWavFile(handle, descriptor);
      this.database.exec('BEGIN IMMEDIATE');
      try {
        this.database.prepare(`INSERT INTO render_cache_files VALUES (?,?,?,?,?,?,?)
          ON CONFLICT(key) DO UPDATE SET format=excluded.format,sample_rate=excluded.sample_rate,channels=excluded.channels,frames=excluded.frames,byte_length=excluded.byte_length,sha256=excluded.sha256`)
          .run(key, descriptor.format, descriptor.sampleRate, descriptor.channels, descriptor.frames, descriptor.byteLength, descriptor.sha256);
        this.database.prepare('INSERT OR REPLACE INTO renders VALUES (?,?,?,?,?)').run(key, projectId, revision, profile, new Date().toISOString());
        this.database.exec('COMMIT');
      } catch (error) {
        this.database.exec('ROLLBACK');
        throw error;
      }
      return { handle, descriptor };
    } catch (error) {
      await handle?.close();
      if (published) await rm(path, { force: true });
      throw error;
    } finally {
      if (ownsTemporary) await rm(temporary, { force: true });
    }
  }
}
