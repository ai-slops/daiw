import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, stat, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { ChatStore } from "../app/chat/store.mjs";

test("chat storage is protected and marks unfinished turns interrupted after restart", async (t) => {
	const directory = await mkdtemp(join(tmpdir(), "daiw-chat-store-"));
	t.after(() => rm(directory, { recursive: true, force: true }));
	let store = new ChatStore(directory);
	const owner = store.owner("browser-secret", true);
	const accountId = store.saveAccount(owner, { clientId: "oaiapp_fixture", subject: "subject-1", profile: { label: "Fixture" }, credentials: { accessToken: "secret" } });
	const session = store.createSession(owner, { projectId: "project-1", accountId, model: "gpt-test" });
	const turn = store.createTurn(session.id, crypto.randomUUID(), "hello", "revision-1");
	store.close();
	store = new ChatStore(directory);
	t.after(() => store.close());
	const loaded = store.turn(turn.id);
	assert.equal(loaded.status, "interrupted");
	assert.equal(loaded.error.code, "HostRestarted");
	assert.equal(store.events(turn.id, 0).at(-1).name, "turn.interrupted");
	assert.equal(store.turns(session.id)[0].lastEventId, 1);
	assert.equal((await stat(directory)).mode & 0o777, 0o700);
	assert.equal((await stat(join(directory, "chat.sqlite"))).mode & 0o777, 0o600);
	for (const suffix of ["-wal", "-shm"]) assert.equal((await stat(join(directory, `chat.sqlite${suffix}`))).mode & 0o777, 0o600);
});

test("chat storage rejects symlink directories", async (t) => {
	const root = await mkdtemp(join(tmpdir(), "daiw-chat-symlink-"));
	const target = join(root, "target");
	const linked = join(root, "linked");
	await mkdir(target);
	await symlink(target, linked, "dir");
	t.after(() => rm(root, { recursive: true, force: true }));
	assert.throws(() => new ChatStore(linked), { code: "UnsafeStoragePath" });
});
