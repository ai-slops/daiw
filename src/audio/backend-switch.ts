export type AudioBackend = "browser" | "native";

type BackendActions = Readonly<{
	stopBrowser: () => void;
	releaseNative: () => Promise<void>;
}>;

export class AudioBackendSwitcher {
	#backend: AudioBackend;
	#tail: Promise<void> = Promise.resolve();

	public constructor(initial: AudioBackend) {
		this.#backend = initial;
	}

	public get backend(): AudioBackend {
		return this.#backend;
	}

	public switch(
		next: AudioBackend,
		actions: BackendActions,
	): Promise<AudioBackend> {
		const operation = this.#tail.then(async () => {
			if (next === this.#backend) return this.#backend;
			if (this.#backend === "browser") actions.stopBrowser();
			else await actions.releaseNative();
			this.#backend = next;
			return this.#backend;
		});
		this.#tail = operation.then(
			() => undefined,
			() => undefined,
		);
		return operation;
	}
}

export class UnsupportedAudioCapabilityError extends Error {
	public readonly code = "UnsupportedAudioCapability";

	public constructor(capability: "ab" | "loudnessMatching") {
		super(`Native output does not support ${capability}.`);
		this.name = "UnsupportedAudioCapabilityError";
	}
}

export class AudioBackendReleaseError extends Error {
	public readonly code = "AudioBackendRelease";

	public constructor() {
		super("Native output could not be released.");
		this.name = "AudioBackendReleaseError";
	}
}
