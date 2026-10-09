import type { AuditionPreparer } from "./audition-preparer";
import type { BoundaryGain } from "./boundary-gain";
import { GenerationGate } from "./generation";

export class TransportIntent {
	readonly #generation = new GenerationGate();
	readonly #preparer: AuditionPreparer;
	readonly #gain: BoundaryGain;

	public constructor(preparer: AuditionPreparer, gain: BoundaryGain) {
		this.#preparer = preparer;
		this.#gain = gain;
	}

	public get current(): number {
		return this.#generation.current;
	}
	public get signal(): AbortSignal {
		return this.#generation.signal;
	}

	public cancelPending(): void {
		this.#preparer.cancel();
		this.#gain.cancel();
	}
	public renewSystem(): number {
		return this.#generation.renew();
	}
	public renewUser(): number {
		this.cancelPending();
		return this.#generation.renew();
	}

	public assert(
		operation: number,
		capturedRevision: string,
		currentRevision: string | null,
	): void {
		this.#generation.assert(operation, capturedRevision, currentRevision);
	}
}
