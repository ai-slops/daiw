import type { PreparedAuditions } from "./audition-preparer";
import {
	auditionGain,
	requiredStream,
	streamingMetrics,
} from "./engine-observation";
import { AudioEngineError } from "./errors";
import type { StreamBranch } from "./stream-branch";
import { StreamBufferBudget } from "./stream-budget";
import type { AuditionSide } from "./types";

export class EngineStreamSet {
	public readonly installed = new Map<AuditionSide, StreamBranch>();
	public readonly budget = new StreamBufferBudget();
	readonly #retiring = new Set<StreamBranch>();

	public replace(prepared: PreparedAuditions): readonly StreamBranch[] {
		const [reference, candidate] = prepared.streams;
		if (reference === undefined)
			throw new AudioEngineError("reference render was not prepared");
		const obsolete = [...this.installed.values()];
		this.installed.clear();
		this.installed.set("a", reference);
		if (candidate !== undefined) this.installed.set("b", candidate);
		return obsolete;
	}

	public retainObsolete(
		obsolete: readonly StreamBranch[],
		hasScheduled: (stream: StreamBranch) => boolean,
	): void {
		for (const stream of obsolete) {
			if (hasScheduled(stream)) this.#retiring.add(stream);
			else stream.clear();
		}
	}

	public releaseRetired(stream: StreamBranch): void {
		if (!this.#retiring.delete(stream)) return;
		stream.clear();
	}

	public metrics() {
		return streamingMetrics(this.installed, this.budget);
	}
	public required(side: AuditionSide | null): StreamBranch {
		return requiredStream(this.installed, side);
	}
	public gain(side: AuditionSide | null, enabled: boolean): number {
		return auditionGain(this.installed, side, enabled);
	}

	public clear(): void {
		for (const stream of this.installed.values()) stream.clear();
		for (const stream of this.#retiring) stream.clear();
		this.installed.clear();
		this.#retiring.clear();
	}
}
