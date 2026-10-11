const { randomUUID } = require("node:crypto");
const { z } = require("zod");

const CHANNELS = Object.freeze({
	capabilities: "daiw-native:capabilities",
	listDevices: "daiw-native:list-devices",
	prepare: "daiw-native:prepare",
	play: "daiw-native:play",
	pause: "daiw-native:pause",
	stop: "daiw-native:stop",
	seek: "daiw-native:seek",
	setLoop: "daiw-native:set-loop",
	release: "daiw-native:release",
	state: "daiw-native:state",
});
const STATE_EVENT = "daiw-native:state-changed";
const token = z.string().min(20).max(128);
const payloads = Object.freeze({
	capabilities: z.object({}).strict(),
	listDevices: z.object({}).strict(),
	prepare: z
		.object({
			projectId: z.string().min(1).max(160),
			expectedRevision: z.string().min(1),
			deviceToken: token.optional(),
		})
		.strict(),
	play: z.object({ sessionToken: token }).strict(),
	pause: z.object({ sessionToken: token }).strict(),
	stop: z.object({ sessionToken: token }).strict(),
	state: z.object({ sessionToken: token }).strict(),
	seek: z
		.object({ sessionToken: token, frame: z.number().int().nonnegative() })
		.strict(),
	setLoop: z
		.object({
			sessionToken: token,
			loop: z
				.object({
					start: z.number().int().nonnegative(),
					end: z.number().int().positive(),
				})
				.strict()
				.nullable(),
		})
		.strict(),
	release: z.object({ sessionToken: token }).strict(),
});
const childMessage = z.union([
	z
		.object({
			channel: z.literal("native-audio"),
			responseTo: z.uuid(),
			ok: z.literal(true),
			result: z.unknown(),
		})
		.strict(),
	z
		.object({
			channel: z.literal("native-audio"),
			responseTo: z.uuid(),
			ok: z.literal(false),
			error: z.object({ code: z.string(), message: z.string() }).strict(),
		})
		.strict(),
	z
		.object({
			channel: z.literal("native-audio"),
			clientId: z.string(),
			event: z.literal("state"),
			state: z.unknown(),
		})
		.strict(),
]);

class DesktopIpcError extends Error {
	constructor(code, message) {
		super(message);
		this.name = "DesktopIpcError";
		this.code = code;
	}
}

function isTrustedFrame(event, origin) {
	return (
		event.senderFrame === event.sender.mainFrame &&
		new URL(event.senderFrame.url).origin === origin
	);
}

class NativeServiceBridge {
	#child;
	#requestTimeoutMs;
	#pending = new Map();
	#clients = new Map();
	constructor(child, { requestTimeoutMs = 30_000 } = {}) {
		this.#child = child;
		this.#requestTimeoutMs = requestTimeoutMs;
		child.on("message", (input) => {
			const parsed = childMessage.safeParse(input);
			if (!parsed.success) return;
			const message = parsed.data;
			if ("event" in message)
				this.#clients.get(message.clientId)?.send(STATE_EVENT, message.state);
			else {
				const pending = this.#pending.get(message.responseTo);
				if (pending === undefined) return;
				this.#pending.delete(message.responseTo);
				clearTimeout(pending.timeout);
				if (message.ok) pending.resolve(message.result);
				else
					pending.reject(
						new DesktopIpcError(message.error.code, message.error.message),
					);
			}
		});
		child.once("close", () => {
			for (const pending of this.#pending.values()) {
				clearTimeout(pending.timeout);
				pending.reject(
					new DesktopIpcError("ServiceClosed", "native audio host closed"),
				);
			}
			this.#pending.clear();
		});
		child.on("error", () => {
			for (const pending of this.#pending.values()) {
				clearTimeout(pending.timeout);
				pending.reject(
					new DesktopIpcError("ServiceClosed", "native audio host closed"),
				);
			}
			this.#pending.clear();
		});
	}
	attach(clientId, webContents) {
		this.#clients.set(clientId, webContents);
	}
	async releaseClient(clientId) {
		this.#clients.delete(clientId);
		if (this.#child.connected)
			await this.request(clientId, "releaseClient", {});
	}
	request(clientId, method, args) {
		if (!this.#child.connected)
			return Promise.reject(
				new DesktopIpcError(
					"ServiceClosed",
					"native audio host is unavailable",
				),
			);
		if (this.#pending.size >= 32)
			return Promise.reject(
				new DesktopIpcError("Busy", "desktop IPC queue is full"),
			);
		const id = randomUUID();
		return new Promise((resolve, reject) => {
			const timeout = setTimeout(() => {
				this.#pending.delete(id);
				if (method === "prepare" && this.#child.connected)
					this.#child.send(
						{
							channel: "native-audio",
							id: randomUUID(),
							clientId,
							method: "cancel",
							args: { requestId: id },
						},
						undefined,
						undefined,
						() => undefined,
					);
				reject(new DesktopIpcError("Timeout", "native audio host timed out"));
			}, this.#requestTimeoutMs);
			this.#pending.set(id, { resolve, reject, timeout });
			this.#child.send(
				{ channel: "native-audio", id, clientId, method, args },
				undefined,
				undefined,
				(error) => {
					if (!error || !this.#pending.delete(id)) return;
					clearTimeout(timeout);
					reject(
						new DesktopIpcError("ServiceClosed", "native audio host closed"),
					);
				},
			);
		});
	}
	async close() {
		await Promise.all(
			[...this.#clients.keys()].map((id) => this.releaseClient(id)),
		);
	}
}

function registerNativeAudioHandlers({ ipcMain, service, origin }) {
	const clients = new Map();
	const releaseCurrent = (senderId, clientId) => {
		if (clients.get(senderId) !== clientId) return;
		clients.delete(senderId);
		void Promise.resolve(service.releaseClient?.(clientId)).catch(
			() => undefined,
		);
	};
	const clientFor = (event) => {
		if (!isTrustedFrame(event, origin))
			throw new DesktopIpcError(
				"UntrustedFrame",
				"native audio IPC requires the trusted application main frame",
			);
		let clientId = clients.get(event.sender.id);
		if (clientId !== undefined) return clientId;
		clientId = randomUUID();
		clients.set(event.sender.id, clientId);
		service.attach?.(clientId, event.sender);
		const release = () => releaseCurrent(event.sender.id, clientId);
		event.sender.once("destroyed", release);
		event.sender.once("render-process-gone", release);
		event.sender.once(
			"did-start-navigation",
			(_navigationEvent, _url, _inPlace, isMainFrame) => {
				if (isMainFrame) release();
			},
		);
		return clientId;
	};
	for (const [method, channel] of Object.entries(CHANNELS)) {
		ipcMain.handle(channel, async (event, input = {}) =>
			service.request(clientFor(event), method, payloads[method].parse(input)),
		);
	}
}

module.exports = {
	CHANNELS,
	STATE_EVENT,
	DesktopIpcError,
	NativeServiceBridge,
	isTrustedFrame,
	registerNativeAudioHandlers,
};
