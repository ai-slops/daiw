import { AudioEngineError } from "./errors";
import { snapshotRevisionMatches } from "./server-render";

export class GenerationGate {
	#value = 0;
	#controller = new AbortController();

	public get current(): number {
		return this.#value;
	}
	public get signal(): AbortSignal {
		return this.#controller.signal;
	}

	public renew(): number {
		this.#controller.abort();
		this.#controller = new AbortController();
		this.#value += 1;
		return this.#value;
	}

	public assert(
		operation: number,
		capturedRevision: string,
		currentRevision: string | null,
	): void {
		if (
			operation !== this.#value ||
			!snapshotRevisionMatches(capturedRevision, currentRevision)
		) {
			throw new AudioEngineError("snapshot changed while preparing audio");
		}
	}
}
