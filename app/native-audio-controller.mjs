import { randomBytes, randomUUID } from "node:crypto";
import { NativeDeviceRegistry } from "./native-audio-devices.mjs";
import { NativeAudioError, NativeAudioWorker } from "./native-audio-worker.mjs";

const token = () => randomBytes(24).toString("base64url");
export class NativeAudioController {
	#binary;
	#media;
	#service;
	#headless;
	#onState;
	#clients = new Map();
	#devices;

	constructor({
		binary,
		media,
		service,
		onState = () => undefined,
		headlessCallbackDriver = false,
	}) {
		this.#binary = binary;
		this.#media = media;
		this.#service = service;
		this.#onState = onState;
		this.#headless = headlessCallbackDriver;
		this.#devices = new NativeDeviceRegistry(binary);
	}

	async capabilities() {
		return this.#devices.capabilities();
	}

	async listDevices(clientId) {
		return this.#devices.list(clientId);
	}

	async prepare(clientId, request, requestId) {
		this.#assertRevision(request.projectId, request.expectedRevision);
		if (!this.#headless && request.deviceToken === undefined)
			throw new NativeAudioError(
				"InvalidDevice",
				"a supported output device token is required",
			);
		const selector =
			request.deviceToken === undefined
				? undefined
				: this.#devices.selector(clientId, request.deviceToken);
		const generation = randomUUID();
		const sessionToken = token();
		const abort = new AbortController();
		const previous = this.#clients.get(clientId);
		this.#clients.set(clientId, {
			generation,
			requestId,
			abort,
			worker: null,
			sessionToken,
			projectId: request.projectId,
			expectedRevision: request.expectedRevision,
		});
		previous?.abort.abort();
		await previous?.worker?.close();
		this.#assertCurrent(clientId, generation);
		let rendered;
		let worker;
		try {
			rendered = await this.#media.render(
				request.projectId,
				undefined,
				request.expectedRevision,
				abort.signal,
			);
			this.#assertCurrent(clientId, generation);
			this.#assertRevision(request.projectId, request.expectedRevision);
			const args = [
				"--control-wav",
				rendered.managedPath,
				"--sha256",
				rendered.descriptor.sha256,
			];
			if (selector !== undefined) {
				args.push(
					"--device-index",
					String(selector.index),
					"--device-sha256",
					selector.digest,
				);
			}
			if (this.#headless) args.push("--headless-callback-driver");
			worker = await NativeAudioWorker.start({
				binary: this.#binary,
				args,
				signal: abort.signal,
				onState: (state) => {
					const current = this.#clients.get(clientId);
					if (current?.generation === generation)
						this.#onState(clientId, {
							sessionToken,
							projectId: request.projectId,
							expectedRevision: request.expectedRevision,
							state,
						});
				},
				onTerminal: (error) => {
					const current = this.#clients.get(clientId);
					if (current?.generation !== generation || current.worker !== worker)
						return;
					this.#clients.delete(clientId);
					this.#onState(clientId, {
						sessionToken,
						projectId: request.projectId,
						expectedRevision: request.expectedRevision,
						fault: { code: error.code, message: error.message },
					});
				},
			});
			this.#assertCurrent(clientId, generation);
			this.#assertRevision(request.projectId, request.expectedRevision);
			this.#clients.set(clientId, {
				generation,
				requestId,
				abort,
				worker,
				sessionToken,
				projectId: request.projectId,
				expectedRevision: request.expectedRevision,
			});
			return { sessionToken };
		} catch (error) {
			const current = this.#clients.get(clientId);
			if (current?.generation === generation) this.#clients.delete(clientId);
			await worker?.close();
			if (current?.generation !== generation)
				throw new NativeAudioError(
					"Superseded",
					"native audio preparation was superseded",
				);
			throw error;
		} finally {
			await rendered?.handle.close();
		}
	}

	async command(clientId, sessionToken, command) {
		const session = this.#session(clientId, sessionToken);
		if (command.type === "play")
			this.#assertRevision(session.projectId, session.expectedRevision);
		return session.worker.command(command);
	}

	async advanceForTest(clientId, sessionToken, frames) {
		if (!this.#headless)
			throw new NativeAudioError(
				"UnsupportedCommand",
				"headless advance is unavailable",
			);
		return this.command(clientId, sessionToken, { type: "advance", frames });
	}

	async release(clientId, sessionToken) {
		this.#session(clientId, sessionToken);
		await this.#releaseSession(clientId);
	}

	async releaseClient(clientId) {
		this.#devices.releaseClient(clientId);
		await this.#releaseSession(clientId);
	}

	async cancel(clientId, requestId) {
		if (this.#clients.get(clientId)?.requestId === requestId)
			await this.#releaseSession(clientId);
	}

	async #releaseSession(clientId) {
		const current = this.#clients.get(clientId);
		if (current === undefined) return;
		this.#clients.delete(clientId);
		current.abort.abort();
		await current.worker?.close();
	}

	async close() {
		this.#devices.clear();
		await Promise.all(
			[...this.#clients.keys()].map((clientId) =>
				this.#releaseSession(clientId),
			),
		);
	}

	#session(clientId, sessionToken) {
		const current = this.#clients.get(clientId);
		if (current?.worker === null || current?.sessionToken !== sessionToken)
			throw new NativeAudioError(
				"InvalidSession",
				"native audio session is unavailable",
			);
		return current;
	}

	#assertRevision(projectId, expectedRevision) {
		if (this.#service.store.get(projectId).revision !== expectedRevision)
			throw new NativeAudioError(
				"StaleRevision",
				"project revision changed before native playback",
			);
	}

	#assertCurrent(clientId, generation) {
		if (this.#clients.get(clientId)?.generation !== generation)
			throw new NativeAudioError(
				"Superseded",
				"native audio preparation was superseded",
			);
	}
}
