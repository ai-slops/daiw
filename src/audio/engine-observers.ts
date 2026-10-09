import { updateMeter } from "./analysis";
import type { AudioEngineFault } from "./errors";
import type {
	AudioEngineState,
	MasterMeter,
	StateListener,
	TransportStatus,
} from "./types";

export class EngineObservers {
	readonly #listeners = new Set<StateListener>();
	readonly #faultListeners = new Set<(fault: AudioEngineFault) => void>();
	readonly #analyser: AnalyserNode;
	readonly #samples: Float32Array<ArrayBuffer>;
	readonly #meter = { peak: 0, rms: 0, peakDb: -Infinity, rmsDb: -Infinity };

	public constructor(context: AudioContext) {
		this.#analyser = context.createAnalyser();
		this.#analyser.fftSize = 2_048;
		this.#analyser.smoothingTimeConstant = 0;
		this.#analyser.connect(context.destination);
		this.#samples = new Float32Array(this.#analyser.fftSize);
	}

	public get input(): AnalyserNode {
		return this.#analyser;
	}

	public meter(status: TransportStatus): MasterMeter {
		if (status === "playing")
			this.#analyser.getFloatTimeDomainData(this.#samples);
		else this.#samples.fill(0);
		return updateMeter(this.#samples, this.#meter);
	}

	public subscribe(
		listener: StateListener,
		state: AudioEngineState,
	): () => void {
		this.#listeners.add(listener);
		listener(state);
		return () => this.#listeners.delete(listener);
	}

	public subscribeFault(
		listener: (fault: AudioEngineFault) => void,
	): () => void {
		this.#faultListeners.add(listener);
		return () => this.#faultListeners.delete(listener);
	}

	public emit(state: AudioEngineState): void {
		for (const listener of this.#listeners) listener(state);
	}

	public emitFault(fault: AudioEngineFault): void {
		for (const listener of this.#faultListeners) listener(fault);
	}

	public dispose(): void {
		this.#listeners.clear();
		this.#faultListeners.clear();
		this.#analyser.disconnect();
	}
}
