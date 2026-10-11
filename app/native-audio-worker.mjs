import { spawn } from "node:child_process";
import { nativeAudioOutputSchema } from "./native-audio-protocol.mjs";

export class NativeAudioError extends Error {
	constructor(code, message) {
		super(message);
		this.name = "NativeAudioError";
		this.code = code;
	}
}

const delay = (milliseconds) =>
	new Promise((resolve) => setTimeout(resolve, milliseconds));

export class NativeAudioWorker {
	#child;
	#commandTimeoutMs;
	#nextId = 1;
	#pending = new Map();
	#ready;
	#closed;
	#buffer = "";
	#stderr = "";
	#onState;
	#onTerminal;
	#resolveReady;
	#rejectReady;
	#terminalError;
	#closePromise;
	#isReady = false;
	#isClosing = false;

	static async start({
		binary,
		args,
		onState,
		onTerminal,
		signal,
		commandTimeoutMs = 5_000,
	}) {
		const worker = new NativeAudioWorker(
			binary,
			args,
			onState,
			onTerminal,
			commandTimeoutMs,
		);
		let rejectAbort;
		const aborted = new Promise((_, reject) => {
			rejectAbort = reject;
		});
		const abort = () =>
			rejectAbort(
				new NativeAudioError(
					"Superseded",
					"native audio preparation was superseded",
				),
			);
		signal?.addEventListener("abort", abort, { once: true });
		try {
			if (signal?.aborted) abort();
			await Promise.race([worker.#ready, aborted]);
			return worker;
		} catch (error) {
			await worker.close();
			throw error;
		} finally {
			signal?.removeEventListener("abort", abort);
		}
	}

	constructor(binary, args, onState, onTerminal, commandTimeoutMs) {
		this.#onState = onState;
		this.#onTerminal = onTerminal;
		this.#commandTimeoutMs = commandTimeoutMs;
		this.#child = spawn(binary, args, { stdio: ["pipe", "pipe", "pipe"] });
		this.#ready = new Promise((resolve, reject) => {
			this.#resolveReady = resolve;
			this.#rejectReady = reject;
		});
		this.#closed = new Promise((resolve) => this.#child.once("close", resolve));
		this.#child.stdout.on("data", (chunk) => this.#read(chunk));
		this.#child.stderr.on("data", (chunk) => {
			this.#stderr = `${this.#stderr}${chunk}`.slice(-8192);
		});
		this.#child.once("error", (error) =>
			this.#fail(new NativeAudioError("WorkerStart", error.message)),
		);
		this.#child.stdin.on("error", () =>
			this.#fail(
				new NativeAudioError("WorkerClosed", "native audio input pipe closed"),
			),
		);
		this.#child.once("close", () =>
			this.#fail(
				new NativeAudioError(
					"WorkerClosed",
					this.#stderr || "native audio worker closed",
				),
			),
		);
	}

	async command(command) {
		if (
			this.#terminalError !== undefined ||
			this.#child.exitCode !== null ||
			this.#child.signalCode !== null
		)
			throw (
				this.#terminalError ??
				new NativeAudioError("WorkerClosed", "native audio worker closed")
			);
		if (this.#pending.size >= 16)
			throw new NativeAudioError("Busy", "native audio command queue is full");
		const id = this.#nextId++;
		const line = `${JSON.stringify({ v: 1, id, ...command })}\n`;
		if (Buffer.byteLength(line) > 4096)
			throw new NativeAudioError(
				"CommandTooLarge",
				"native audio command exceeds 4096 bytes",
			);
		const result = new Promise((resolve, reject) => {
			const timer = setTimeout(
				() =>
					this.#fail(
						new NativeAudioError(
							"CommandTimeout",
							"native audio command timed out",
						),
					),
				this.#commandTimeoutMs,
			);
			this.#pending.set(id, { resolve, reject, timer });
		});
		this.#child.stdin.write(line, (error) => {
			if (error !== null && error !== undefined)
				this.#fail(
					new NativeAudioError(
						"WorkerClosed",
						"native audio input pipe closed",
					),
				);
		});
		return result;
	}

	close() {
		this.#closePromise ??= this.#close();
		return this.#closePromise;
	}

	async #close() {
		this.#isClosing = true;
		if (this.#child.exitCode !== null || this.#child.signalCode !== null)
			return;
		try {
			await Promise.race([this.command({ type: "shutdown" }), delay(500)]);
		} catch (error) {
			if (!(error instanceof NativeAudioError)) throw error;
		}
		this.#child.stdin.end();
		await Promise.race([this.#closed, delay(500)]);
		if (this.#child.exitCode === null && this.#child.signalCode === null)
			this.#child.kill("SIGTERM");
		await Promise.race([this.#closed, delay(500)]);
		if (this.#child.exitCode === null && this.#child.signalCode === null)
			this.#child.kill("SIGKILL");
		await this.#closed;
	}

	#read(chunk) {
		this.#buffer += chunk.toString("utf8");
		for (;;) {
			const newline = this.#buffer.indexOf("\n");
			if (newline < 0) break;
			const line = this.#buffer.slice(0, newline);
			this.#buffer = this.#buffer.slice(newline + 1);
			if (Buffer.byteLength(line) > 4096)
				return this.#fail(
					new NativeAudioError(
						"ProtocolOverflow",
						"native audio output line overflow",
					),
				);
			let output;
			try {
				output = nativeAudioOutputSchema.parse(JSON.parse(line));
			} catch (error) {
				return this.#fail(
					new NativeAudioError(
						"InvalidProtocol",
						error instanceof Error
							? error.message
							: "invalid native audio output",
					),
				);
			}
			this.#dispatch(output);
		}
		if (Buffer.byteLength(this.#buffer) > 4096)
			this.#fail(
				new NativeAudioError(
					"ProtocolOverflow",
					"native audio output fragment overflow",
				),
			);
	}

	#dispatch(output) {
		switch (output.type) {
			case "ready":
				this.#isReady = true;
				this.#resolveReady(output.frames);
				return;
			case "state":
				this.#onState(output.state);
				return;
			case "fatal":
				this.#fail(
					new NativeAudioError(output.error.code, output.error.message),
				);
				return;
			case "response": {
				const pending = this.#pending.get(output.id);
				if (pending === undefined) return;
				this.#pending.delete(output.id);
				clearTimeout(pending.timer);
				if (output.ok) pending.resolve(output.state);
				else
					pending.reject(
						new NativeAudioError(output.error.code, output.error.message),
					);
				return;
			}
		}
	}

	#fail(error) {
		if (this.#terminalError !== undefined) return;
		this.#terminalError = error;
		this.#rejectReady(error);
		for (const pending of this.#pending.values()) {
			clearTimeout(pending.timer);
			pending.reject(error);
		}
		this.#pending.clear();
		if (this.#isReady && !this.#isClosing) this.#onTerminal?.(error);
		if (this.#child.exitCode === null && this.#child.signalCode === null)
			this.#child.kill("SIGTERM");
	}
}
