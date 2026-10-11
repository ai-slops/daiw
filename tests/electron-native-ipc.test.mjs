import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { createRequire } from "node:module";
import { test } from "node:test";
import {
	isTrustedFrame,
	NativeServiceBridge,
	registerNativeAudioHandlers,
} from "../desktop/native-ipc.cjs";

const origin = "http://127.0.0.1:3333";

test("Electron trust boundary requires the owned main frame at the exact app origin", () => {
	// Given
	const mainFrame = { url: `${origin}/` };
	const sender = { mainFrame };
	// When / Then
	assert.equal(
		isTrustedFrame({ sender, senderFrame: mainFrame }, origin),
		true,
	);
	assert.equal(
		isTrustedFrame({ sender, senderFrame: { url: `${origin}/frame` } }, origin),
		false,
	);
	assert.equal(
		isTrustedFrame(
			{ sender, senderFrame: { url: "https://example.test/" } },
			origin,
		),
		false,
	);
});

test("Electron registers named capabilities and never forwards renderer client identity", async () => {
	// Given
	const handlers = new Map();
	const calls = [];
	const ipcMain = {
		handle: (channel, handler) => handlers.set(channel, handler),
	};
	const service = {
		request: async (clientId, method, args) => {
			calls.push({ clientId, method, args });
			return { ok: true };
		},
	};
	registerNativeAudioHandlers({ ipcMain, service, origin });
	const frame = { url: `${origin}/` };
	const event = {
		sender: { id: 17, mainFrame: frame, once: () => undefined },
		senderFrame: frame,
	};
	// When
	await handlers.get("daiw-native:list-devices")(event);
	// Then
	assert.deepEqual([...handlers.keys()].sort(), [
		"daiw-native:capabilities",
		"daiw-native:list-devices",
		"daiw-native:pause",
		"daiw-native:play",
		"daiw-native:prepare",
		"daiw-native:release",
		"daiw-native:seek",
		"daiw-native:set-loop",
		"daiw-native:state",
		"daiw-native:stop",
	]);
	assert.equal(calls[0].method, "listDevices");
	assert.notEqual(calls[0].clientId, "17");
	assert.equal("clientId" in calls[0].args, false);
	await assert.rejects(
		handlers.get("daiw-native:capabilities")({
			...event,
			senderFrame: { url: `${origin}/subframe` },
		}),
		{ code: "UntrustedFrame" },
	);
});

test("preload exposes individual methods and state subscription without generic send", () => {
	// Given
	const exposed = {};
	const listeners = new Map();
	const electron = {
		contextBridge: {
			exposeInMainWorld: (name, value) => {
				exposed[name] = value;
			},
		},
		ipcRenderer: {
			invoke: (channel, payload) => Promise.resolve({ channel, payload }),
			on: (channel, listener) => listeners.set(channel, listener),
			removeListener: (channel) => listeners.delete(channel),
		},
	};
	const require = createRequire(import.meta.url);
	const Module = require("node:module");
	const originalLoad = Module._load;
	Module._load = (request, parent, isMain) =>
		request === "electron" ? electron : originalLoad(request, parent, isMain);
	try {
		delete require.cache[require.resolve("../desktop/preload.cjs")];
		require("../desktop/preload.cjs");
	} finally {
		Module._load = originalLoad;
	}
	// When / Then
	assert.deepEqual(Object.keys(exposed.daiwNativeAudio).sort(), [
		"capabilities",
		"listDevices",
		"onState",
		"pause",
		"play",
		"prepare",
		"release",
		"seek",
		"setLoop",
		"state",
		"stop",
	]);
	assert.equal("send" in exposed.daiwNativeAudio, false);
});

test("desktop timeout notifies the host to cancel the matching preparation", async () => {
	// Given
	const child = new EventEmitter();
	child.connected = true;
	const sent = [];
	child.send = (message) => sent.push(message);
	const service = new NativeServiceBridge(child, { requestTimeoutMs: 20 });
	// When / Then
	await assert.rejects(
		service.request("client-1", "prepare", {
			projectId: "project-1",
			expectedRevision: "revision-1",
		}),
		{ code: "Timeout" },
	);
	assert.equal(sent.length, 2);
	assert.equal(sent[1].method, "cancel");
	assert.equal(sent[1].args.requestId, sent[0].id);
});

test("renderer navigation and process loss release only their current client generation", async () => {
	// Given
	const handlers = new Map();
	const listeners = new Map();
	const releases = [];
	const ipcMain = {
		handle: (channel, handler) => handlers.set(channel, handler),
	};
	const service = {
		attach: () => undefined,
		request: async () => ({ ok: true }),
		releaseClient: async (clientId) => releases.push(clientId),
	};
	registerNativeAudioHandlers({ ipcMain, service, origin });
	const frame = { url: `${origin}/` };
	const sender = {
		id: 17,
		mainFrame: frame,
		once: (event, listener) => {
			const current = listeners.get(event) ?? [];
			listeners.set(event, [...current, listener]);
		},
	};
	const event = { sender, senderFrame: frame };
	await handlers.get("daiw-native:list-devices")(event);
	// When
	listeners.get("did-start-navigation")[0]({}, `${origin}/`, false, true);
	await handlers.get("daiw-native:list-devices")(event);
	for (const listener of listeners.get("render-process-gone")) listener();
	await new Promise((resolvePromise) => setImmediate(resolvePromise));
	// Then
	assert.equal(releases.length, 2);
	assert.notEqual(releases[0], releases[1]);
});

test("disconnect during child send rejects pending work without an unhandled error", async () => {
	// Given
	const child = new EventEmitter();
	child.connected = true;
	child.send = (_message, _sendHandle, _options, callback) => {
		child.connected = false;
		callback(new Error("channel closed"));
		child.emit("error", new Error("channel closed"));
	};
	const service = new NativeServiceBridge(child);
	// When / Then
	await assert.rejects(service.request("client-1", "capabilities", {}), {
		code: "ServiceClosed",
	});
});

test("renderer lifecycle cleanup contains release rejection", async () => {
	// Given
	const handlers = new Map();
	let destroyed;
	const ipcMain = {
		handle: (channel, handler) => handlers.set(channel, handler),
	};
	const service = {
		attach: () => undefined,
		request: async () => ({ ok: true }),
		releaseClient: async () => {
			throw new Error("release failed");
		},
	};
	registerNativeAudioHandlers({ ipcMain, service, origin });
	const frame = { url: `${origin}/` };
	const sender = {
		id: 31,
		mainFrame: frame,
		once: (event, listener) => {
			if (event === "destroyed") destroyed = listener;
		},
	};
	await handlers.get("daiw-native:list-devices")({
		sender,
		senderFrame: frame,
	});
	// When
	destroyed();
	await new Promise((resolvePromise) => setImmediate(resolvePromise));
	// Then
	assert.equal(true, true);
});
