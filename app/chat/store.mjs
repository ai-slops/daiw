import { createHash, randomUUID } from "node:crypto";
import { chmodSync, existsSync, lstatSync, mkdirSync, realpathSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.mjs";

const encode = (value) => JSON.stringify(value);
const decode = (value) => JSON.parse(value);
const ownerHash = (sid) => createHash("sha256").update(sid).digest("hex");
const terminalTtl = 10 * 60_000;

function protectPath(path, mode) {
	if (process.platform !== "win32" && existsSync(path)) chmodSync(path, mode);
}

function prepareDirectory(directory) {
	if (!isAbsolute(directory)) throw new ServiceError("UnsafeStoragePath", "Chat storage must use an absolute path.", 500);
	const absolute = resolve(directory);
	if (existsSync(absolute) && lstatSync(absolute).isSymbolicLink()) throw new ServiceError("UnsafeStoragePath", "Chat storage cannot use a symbolic link.", 500);
	mkdirSync(absolute, { recursive: true, mode: 0o700 });
	if (realpathSync(absolute) !== absolute) throw new ServiceError("UnsafeStoragePath", "Chat storage cannot traverse symbolic links.", 500);
	protectPath(absolute, 0o700);
	return absolute;
}

export class ChatStore {
	constructor(directory) {
		const absolute = prepareDirectory(directory);
		this.path = join(absolute, "chat.sqlite");
		if (existsSync(this.path) && lstatSync(this.path).isSymbolicLink()) throw new ServiceError("UnsafeStoragePath", "Chat storage database cannot be a symbolic link.", 500);
		this.db = new DatabaseSync(this.path);
		this.db.exec(`PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
			CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
			CREATE TABLE IF NOT EXISTS owners (hash TEXT PRIMARY KEY, created_at INTEGER NOT NULL);
			CREATE TABLE IF NOT EXISTS registrations (owner_hash TEXT NOT NULL, client_id TEXT NOT NULL, account_id TEXT, created_at INTEGER NOT NULL, PRIMARY KEY(owner_hash,client_id));
			CREATE TABLE IF NOT EXISTS accounts (id TEXT PRIMARY KEY, owner_hash TEXT NOT NULL, client_id TEXT NOT NULL, subject TEXT NOT NULL, profile TEXT NOT NULL, credentials TEXT, connected INTEGER NOT NULL, UNIQUE(owner_hash,client_id,subject));
			CREATE TABLE IF NOT EXISTS owner_settings (owner_hash TEXT PRIMARY KEY, active_account_id TEXT);
			CREATE TABLE IF NOT EXISTS pending_logins (id TEXT PRIMARY KEY, owner_hash TEXT NOT NULL, data TEXT NOT NULL, expires_at INTEGER NOT NULL);
			CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, owner_hash TEXT NOT NULL, project_id TEXT NOT NULL, account_id TEXT NOT NULL, model TEXT NOT NULL, title TEXT NOT NULL, updated_at TEXT NOT NULL);
			CREATE TABLE IF NOT EXISTS turns (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, client_turn_id TEXT NOT NULL, text TEXT NOT NULL, status TEXT NOT NULL, assistant_text TEXT NOT NULL DEFAULT '', error TEXT, history TEXT NOT NULL DEFAULT '[]', snapshot_revision TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(session_id,client_turn_id));
			CREATE TABLE IF NOT EXISTS events (turn_id TEXT NOT NULL, sequence INTEGER NOT NULL, name TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY(turn_id,sequence));
			CREATE TABLE IF NOT EXISTS tool_results (turn_id TEXT NOT NULL, call_id TEXT NOT NULL, name TEXT NOT NULL, call_hash TEXT NOT NULL, status TEXT NOT NULL, result TEXT, PRIMARY KEY(turn_id,call_id));`);
		this.#interruptRunning();
		this.#protectFiles();
	}
	#protectFiles() { for (const suffix of ["", "-wal", "-shm"]) protectPath(`${this.path}${suffix}`, 0o600); }
	#interruptRunning() {
		const error = { code: "HostRestarted", message: "The host restarted while this turn was running." };
		this.db.exec("BEGIN IMMEDIATE");
		try {
			for (const row of this.db.prepare("SELECT id FROM turns WHERE status='running'").all()) {
				const sequence = (this.db.prepare("SELECT MAX(sequence) value FROM events WHERE turn_id=?").get(row.id)?.value ?? 0) + 1;
				this.db.prepare("INSERT INTO events VALUES (?,?,?,?)").run(row.id, sequence, "turn.interrupted", encode({ turnId: row.id, error }));
				this.db.prepare("UPDATE turns SET status='interrupted',error=? WHERE id=?").run(encode(error), row.id);
			}
			this.db.exec("COMMIT");
		} catch (errorValue) { this.db.exec("ROLLBACK"); throw errorValue; }
	}
	meta(key, factory) { const prior = this.db.prepare("SELECT value FROM meta WHERE key=?").get(key); if (prior) return prior.value; const value = factory(); this.db.prepare("INSERT INTO meta VALUES (?,?)").run(key, value); return value; }
	owner(sid, create = false) { const hash = ownerHash(sid); const found = this.db.prepare("SELECT 1 FROM owners WHERE hash=?").get(hash); if (!found && create) this.db.prepare("INSERT INTO owners VALUES (?,?)").run(hash, Date.now()); return found || create ? hash : undefined; }
	accounts(owner) { return this.db.prepare("SELECT * FROM accounts WHERE owner_hash=? ORDER BY rowid").all(owner).map((row) => ({ ...row, profile: decode(row.profile), credentials: row.credentials ? decode(row.credentials) : null, connected: Boolean(row.connected) })); }
	account(owner, id) { const row = this.db.prepare("SELECT * FROM accounts WHERE owner_hash=? AND id=?").get(owner, id); if (!row) throw new ServiceError("MissingAccount", "ChatGPT account not found.", 404); return { ...row, profile: decode(row.profile), credentials: row.credentials ? decode(row.credentials) : null, connected: Boolean(row.connected) }; }
	registration(owner) { return this.db.prepare("SELECT client_id clientId FROM registrations WHERE owner_hash=? AND account_id IS NULL ORDER BY created_at DESC LIMIT 1").get(owner); }
	saveRegistration(owner, clientId) { this.db.prepare("INSERT INTO registrations VALUES (?,?,NULL,?) ON CONFLICT(owner_hash,client_id) DO NOTHING").run(owner, clientId, Date.now()); }
	saveAccount(owner, registration) {
		const prior = this.db.prepare("SELECT id FROM accounts WHERE owner_hash=? AND client_id=? AND subject=?").get(owner, registration.clientId, registration.subject);
		const id = prior?.id ?? randomUUID();
		this.db.exec("BEGIN IMMEDIATE");
		try {
			this.db.prepare("INSERT INTO accounts VALUES (?,?,?,?,?,?,1) ON CONFLICT(id) DO UPDATE SET profile=excluded.profile,credentials=excluded.credentials,connected=1").run(id, owner, registration.clientId, registration.subject, encode(registration.profile), encode(registration.credentials));
			this.db.prepare("INSERT INTO registrations VALUES (?,?,?,?) ON CONFLICT(owner_hash,client_id) DO UPDATE SET account_id=excluded.account_id").run(owner, registration.clientId, id, Date.now());
			this.db.prepare("INSERT INTO owner_settings VALUES (?,?) ON CONFLICT(owner_hash) DO UPDATE SET active_account_id=excluded.active_account_id").run(owner, id);
			this.db.exec("COMMIT");
		} catch (error) { this.db.exec("ROLLBACK"); throw error; }
		return id;
	}
	active(owner) { return this.db.prepare("SELECT active_account_id id FROM owner_settings WHERE owner_hash=?").get(owner)?.id ?? null; }
	select(owner, id) { this.account(owner, id); this.db.prepare("INSERT INTO owner_settings VALUES (?,?) ON CONFLICT(owner_hash) DO UPDATE SET active_account_id=excluded.active_account_id").run(owner, id); }
	disconnect(owner, id) { this.account(owner, id); this.db.prepare("UPDATE accounts SET credentials=NULL,connected=0 WHERE owner_hash=? AND id=?").run(owner, id); if (this.active(owner) === id) this.db.prepare("UPDATE owner_settings SET active_account_id=NULL WHERE owner_hash=?").run(owner); }
	updateCredentials(owner, id, credentials) { this.db.prepare("UPDATE accounts SET credentials=?,connected=1 WHERE owner_hash=? AND id=?").run(encode(credentials), owner, id); this.db.exec("PRAGMA wal_checkpoint(PASSIVE)"); this.#protectFiles(); }
	quarantineCredentials(owner, id, credentials) {
		this.db.exec("BEGIN IMMEDIATE");
		try {
			this.db.prepare("UPDATE accounts SET credentials=?,connected=0 WHERE owner_hash=? AND id=?").run(encode(credentials), owner, id);
			this.db.prepare("UPDATE owner_settings SET active_account_id=NULL WHERE owner_hash=? AND active_account_id=?").run(owner, id);
			this.db.exec("COMMIT");
		} catch (error) { this.db.exec("ROLLBACK"); throw error; }
		this.db.exec("PRAGMA wal_checkpoint(PASSIVE)"); this.#protectFiles();
	}
	#purgePending() { this.db.prepare("DELETE FROM pending_logins WHERE expires_at<?").run(Date.now()); }
	savePending(id, owner, data) { this.#purgePending(); const pending = { ...data, status: "pending" }; this.db.prepare("INSERT INTO pending_logins VALUES (?,?,?,?)").run(id, owner, encode(pending), data.expiresAt + terminalTtl); }
	pending(id, owner) { this.#purgePending(); const row = this.db.prepare("SELECT data FROM pending_logins WHERE id=? AND owner_hash=?").get(id, owner); if (!row) throw new ServiceError("InvalidLogin", "Login attempt was not found.", 404); const data = decode(row.data); if (data.status === "pending" && data.expiresAt <= Date.now()) return this.finishPending(id, owner, { status: "expired" }); return data; }
	claimPending(id, owner) { this.db.exec("BEGIN IMMEDIATE"); try { const data = this.pending(id, owner); if (data.status !== "pending") throw new ServiceError("InvalidLogin", "Login attempt is no longer pending.", 409); const claimed = { ...data, status: "processing" }; this.db.prepare("UPDATE pending_logins SET data=? WHERE id=? AND owner_hash=?").run(encode(claimed), id, owner); this.db.exec("COMMIT"); return claimed; } catch (error) { this.db.exec("ROLLBACK"); throw error; } }
	finishPending(id, owner, result) { const data = { ...this.pendingRaw(id, owner), ...result }; this.db.prepare("UPDATE pending_logins SET data=?,expires_at=? WHERE id=? AND owner_hash=?").run(encode(data), Date.now() + terminalTtl, id, owner); return data; }
	pendingRaw(id, owner) { const row = this.db.prepare("SELECT data FROM pending_logins WHERE id=? AND owner_hash=?").get(id, owner); if (!row) throw new ServiceError("InvalidLogin", "Login attempt was not found.", 404); return decode(row.data); }
	createSession(owner, value) { const session = { id: randomUUID(), ...value, title: "New chat", updatedAt: new Date().toISOString() }; this.db.prepare("INSERT INTO sessions VALUES (?,?,?,?,?,?,?)").run(session.id, owner, session.projectId, session.accountId, session.model, session.title, session.updatedAt); return session; }
	sessions(owner, projectId, accountId) { return this.db.prepare("SELECT id,project_id projectId,account_id accountId,model,title,updated_at updatedAt FROM sessions WHERE owner_hash=? AND project_id=? AND account_id=? ORDER BY updated_at DESC").all(owner, projectId, accountId); }
	session(owner, id) { const row = this.db.prepare("SELECT id,project_id projectId,account_id accountId,model,title,updated_at updatedAt FROM sessions WHERE owner_hash=? AND id=?").get(owner, id); if (!row) throw new ServiceError("MissingSession", "Chat session not found.", 404); return row; }
	createTurn(sessionId, clientTurnId, text, revision) { const prior = this.db.prepare("SELECT id FROM turns WHERE session_id=? AND client_turn_id=?").get(sessionId, clientTurnId); if (prior) return { id: prior.id, replay: true }; if (this.db.prepare("SELECT 1 FROM turns WHERE session_id=? AND status='running'").get(sessionId)) throw new ServiceError("TurnBusy", "This session already has a running turn.", 409); const id = randomUUID(); this.db.prepare("INSERT INTO turns (id,session_id,client_turn_id,text,status,snapshot_revision,created_at) VALUES (?,?,?,?,'running',?,?)").run(id, sessionId, clientTurnId, text, revision, Date.now()); return { id, replay: false }; }
	appendEvent(turnId, name, data) { const sequence = (this.db.prepare("SELECT MAX(sequence) value FROM events WHERE turn_id=?").get(turnId)?.value ?? 0) + 1; this.db.prepare("INSERT INTO events VALUES (?,?,?,?)").run(turnId, sequence, name, encode(data)); return sequence; }
	appendAssistantText(turnId, text) { this.db.prepare("UPDATE turns SET assistant_text=assistant_text || ? WHERE id=?").run(text, turnId); }
	appendHistory(turnId, items) { const turn = this.turn(turnId); this.db.prepare("UPDATE turns SET history=? WHERE id=?").run(encode([...turn.history, ...items]), turnId); }
	events(turnId, after, limit = 256) { return this.db.prepare("SELECT sequence,name,data FROM events WHERE turn_id=? AND sequence>? ORDER BY sequence LIMIT ?").all(turnId, after, limit).map((row) => ({ ...row, data: decode(row.data) })); }
	turn(id) { const row = this.db.prepare("SELECT * FROM turns WHERE id=?").get(id); if (!row) throw new ServiceError("MissingTurn", "Chat turn not found.", 404); return { ...row, error: row.error ? decode(row.error) : undefined, history: decode(row.history) }; }
	finishTurn(id, status, assistantText, error, eventName, eventData) {
		this.db.exec("BEGIN IMMEDIATE");
		try {
			this.db.prepare("UPDATE turns SET status=?,assistant_text=?,error=? WHERE id=?").run(status, assistantText, error ? encode(error) : null, id);
			const sequence = (this.db.prepare("SELECT MAX(sequence) value FROM events WHERE turn_id=?").get(id)?.value ?? 0) + 1;
			this.db.prepare("INSERT INTO events VALUES (?,?,?,?)").run(id, sequence, eventName, encode(eventData));
			this.db.exec("COMMIT");
			return sequence;
		} catch (errorValue) { this.db.exec("ROLLBACK"); throw errorValue; }
	}
	turns(sessionId) { return this.db.prepare("SELECT t.*,COALESCE(MAX(e.sequence),0) last_event_id FROM turns t LEFT JOIN events e ON e.turn_id=t.id WHERE t.session_id=? GROUP BY t.id ORDER BY t.created_at").all(sessionId).map((row) => ({ id: row.id, text: row.text, status: row.status, assistantText: row.assistant_text, lastEventId: row.last_event_id, ...(row.error ? { error: decode(row.error) } : {}), tools: this.tools(row.id) })); }
	historyGroups(sessionId) { return this.db.prepare("SELECT text,history FROM turns WHERE session_id=? AND status='completed' ORDER BY created_at").all(sessionId).map((row) => [{ role: "user", content: row.text }, ...decode(row.history)]); }
	tool(turnId, callId) { const row = this.db.prepare("SELECT * FROM tool_results WHERE turn_id=? AND call_id=?").get(turnId, callId); return row ? { ...row, result: row.result ? decode(row.result) : undefined } : undefined; }
	startTool(turnId, callId, name, hash) { this.db.prepare("INSERT INTO tool_results VALUES (?,?,?,?,'running',NULL)").run(turnId, callId, name, hash); }
	finishTool(turnId, callId, result) { this.db.prepare("UPDATE tool_results SET status='completed',result=? WHERE turn_id=? AND call_id=?").run(encode(result), turnId, callId); }
	failTool(turnId, callId, error) { this.db.prepare("UPDATE tool_results SET status='failed',result=? WHERE turn_id=? AND call_id=?").run(encode({ output: error }), turnId, callId); }
	tools(turnId) { return this.db.prepare("SELECT call_id callId,name,status,result FROM tool_results WHERE turn_id=? ORDER BY rowid").all(turnId).map((row) => row.result ? { callId: row.callId, name: row.name, status: row.status, result: decode(row.result).output } : { callId: row.callId, name: row.name, status: row.status }); }
	close() { this.db.exec("PRAGMA wal_checkpoint(FULL)"); this.#protectFiles(); this.db.close(); }
}
