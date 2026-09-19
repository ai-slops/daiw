import { DatabaseSync } from 'node:sqlite';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { ServiceError } from './errors.mjs';

export class ProjectStore {
  constructor(directory) {
    mkdirSync(directory, { recursive: true });
    this.db = new DatabaseSync(join(directory, 'state.sqlite'));
    this.db.exec(`PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
      CREATE TABLE IF NOT EXISTS project_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      INSERT OR IGNORE INTO project_meta VALUES ('schemaVersion','1');
      CREATE TABLE IF NOT EXISTS revisions (revision TEXT PRIMARY KEY, project_id TEXT NOT NULL, payload TEXT NOT NULL, hash TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, name TEXT NOT NULL, head TEXT NOT NULL REFERENCES revisions(revision), updated_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS journal (sequence INTEGER PRIMARY KEY, project_id TEXT NOT NULL, request_id TEXT NOT NULL, request_hash TEXT NOT NULL, method TEXT NOT NULL, principal TEXT NOT NULL, base_revision TEXT, revision TEXT NOT NULL, response TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(project_id,request_id));
      CREATE TABLE IF NOT EXISTS undo_stack (project_id TEXT PRIMARY KEY, undo TEXT NOT NULL DEFAULT '[]', redo TEXT NOT NULL DEFAULT '[]');
      CREATE TABLE IF NOT EXISTS assets (id TEXT PRIMARY KEY, hash TEXT NOT NULL, name TEXT NOT NULL, metadata TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS renders (key TEXT PRIMARY KEY, project_id TEXT NOT NULL, revision TEXT NOT NULL, profile TEXT NOT NULL, created_at TEXT NOT NULL);`);
    if (this.db.prepare('SELECT value FROM project_meta WHERE key = ?').get('schemaVersion')?.value !== '1') {
      throw new ServiceError('MigrationRequired', 'Unsupported project database schema.', 500);
    }
  }
  list() {
    return this.db.prepare('SELECT id,name,head AS revision,updated_at AS updatedAt FROM projects ORDER BY updated_at DESC').all();
  }
  get(id) {
    const row = this.db.prepare('SELECT r.payload,r.hash FROM projects p JOIN revisions r ON r.revision=p.head WHERE p.id=?').get(id);
    if (!row) throw new ServiceError('MissingProject', 'Project not found.', 404);
    if (createHash('sha256').update(row.payload).digest('hex') !== row.hash) throw new ServiceError('CorruptProject', 'Project revision integrity check failed.', 422);
    return JSON.parse(row.payload);
  }
  replay(projectId, requestId, hash) {
    const row = projectId
      ? this.db.prepare('SELECT request_hash,response FROM journal WHERE project_id=? AND request_id=?').get(projectId, requestId)
      : this.db.prepare("SELECT request_hash,response FROM journal WHERE method IN ('project.create','project.import') AND request_id=?").get(requestId);
    if (!row) return undefined;
    if (row.request_hash !== hash) throw new ServiceError('IdempotencyConflict', 'This request key was already used for a different command.', 409);
    return JSON.parse(row.response);
  }
  historyTarget(projectId, direction) {
    const stack = this.stacks(projectId)[direction];
    const revision = stack.at(-1);
    if (!revision) throw new ServiceError('EmptyHistory', `Nothing to ${direction}.`, 409);
    const row = this.db.prepare('SELECT payload FROM revisions WHERE revision=? AND project_id=?').get(revision, projectId);
    if (!row) throw new ServiceError('CorruptProject', 'History revision is missing.', 422);
    return JSON.parse(row.payload);
  }
  stacks(projectId) {
    const row = this.db.prepare('SELECT undo,redo FROM undo_stack WHERE project_id=?').get(projectId);
    return row ? { undo: JSON.parse(row.undo), redo: JSON.parse(row.redo) } : { undo: [], redo: [] };
  }
  commit({ response, base, requestId, requestHash, method, principal }) {
    const state = response.state;
    const payload = JSON.stringify(state);
    const hash = createHash('sha256').update(payload).digest('hex');
    const now = new Date().toISOString();
    this.db.exec('BEGIN IMMEDIATE');
    try {
      const current = this.db.prepare('SELECT head FROM projects WHERE id=?').get(state.id);
      if ((current?.head ?? null) !== (base?.revision ?? null)) throw new ServiceError('StaleRevision', 'The project changed. Refresh before applying this command.', 409);
      this.db.prepare('INSERT INTO revisions VALUES (?,?,?,?)').run(state.revision, state.id, payload, hash);
      this.db.prepare('INSERT INTO projects VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,head=excluded.head,updated_at=excluded.updated_at').run(state.id, state.name, state.revision, now);
      this.db.prepare('INSERT INTO journal (project_id,request_id,request_hash,method,principal,base_revision,revision,response,created_at) VALUES (?,?,?,?,?,?,?,?,?)').run(state.id, requestId, requestHash, method, principal, base?.revision ?? null, state.revision, JSON.stringify(response), now);
      const stacks = this.stacks(state.id);
      if (base) {
        switch (method) {
          case 'project.undo': stacks.undo.pop(); stacks.redo.push(base.revision); break;
          case 'project.redo': stacks.redo.pop(); stacks.undo.push(base.revision); break;
          default: stacks.undo.push(base.revision); stacks.redo = [];
        }
      }
      this.db.prepare('INSERT INTO undo_stack VALUES (?,?,?) ON CONFLICT(project_id) DO UPDATE SET undo=excluded.undo,redo=excluded.redo').run(state.id, JSON.stringify(stacks.undo.slice(-100)), JSON.stringify(stacks.redo.slice(-100)));
      this.db.exec('COMMIT');
    } catch (error) {
      this.db.exec('ROLLBACK');
      throw error;
    }
  }
  checkpoint() { this.db.exec('PRAGMA wal_checkpoint(FULL)'); }
  close() { this.checkpoint(); this.db.close(); }
}
