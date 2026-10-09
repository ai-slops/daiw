import { isAbortError } from "./errors";
import { GenerationGate } from "./generation";
import { snapshotFrames } from "./playback-frames";
import { primePreparedPair, settlePreparedPair } from "./prepare-pair";
import { prepareStream } from "./prepare-stream";
import type { ServerRenderRequest } from "./server-render";
import type { StreamBranch } from "./stream-branch";
import type { StreamBufferBudget } from "./stream-budget";
import type { Snapshot } from "./types";

type PreparationHost = Readonly<{
	currentRevision: () => string | null;
	currentFrame: () => number;
}>;
type PreparationPlan = Readonly<{
	snapshot: Snapshot;
	initialFrame: number;
	playing: boolean;
}>;
export type PreparedAuditions = Readonly<{
	streams: readonly StreamBranch[];
	frame: number;
}>;

export class AuditionPreparer {
	readonly #context: AudioContext;
	readonly #generation = new GenerationGate();
	readonly #host: PreparationHost;
	readonly #budget: StreamBufferBudget;

	public constructor(
		context: AudioContext,
		host: PreparationHost,
		budget: StreamBufferBudget,
	) {
		this.#context = context;
		this.#host = host;
		this.#budget = budget;
	}

	public cancel(): void {
		this.#generation.renew();
	}

	public async single(
		request: ServerRenderRequest,
		plan: PreparationPlan,
	): Promise<PreparedAuditions | null> {
		const operation = this.#generation.renew();
		let stream: StreamBranch | null = null;
		try {
			stream = await prepareStream(
				this.#context,
				request,
				this.#streamPlan(plan),
			);
			this.#generation.assert(
				operation,
				plan.snapshot.revision,
				this.#host.currentRevision(),
			);
			const frame = plan.playing
				? this.#host.currentFrame()
				: plan.initialFrame;
			await stream.prime(frame, this.#generation.signal);
			this.#generation.assert(
				operation,
				plan.snapshot.revision,
				this.#host.currentRevision(),
			);
			return {
				streams: [stream],
				frame: plan.playing ? this.#host.currentFrame() : frame,
			};
		} catch (reason) {
			stream?.clear();
			if (operation !== this.#generation.current || isAbortError(reason))
				return null;
			this.#generation.renew();
			throw reason;
		}
	}

	public async pair(
		requests: Readonly<{
			reference: ServerRenderRequest;
			candidate: ServerRenderRequest;
		}>,
		plan: PreparationPlan,
	): Promise<PreparedAuditions | null> {
		const operation = this.#generation.renew();
		let failed = false;
		let pair: readonly [StreamBranch, StreamBranch];
		try {
			pair = await settlePreparedPair(
				prepareStream(
					this.#context,
					requests.reference,
					this.#streamPlan(plan),
				),
				prepareStream(
					this.#context,
					requests.candidate,
					this.#streamPlan(plan),
				),
				() => {
					failed = true;
					if (operation === this.#generation.current) this.#generation.renew();
				},
			);
		} catch (reason) {
			if (
				!failed &&
				(operation !== this.#generation.current || isAbortError(reason))
			)
				return null;
			this.#generation.renew();
			throw reason;
		}
		try {
			this.#generation.assert(
				operation,
				plan.snapshot.revision,
				this.#host.currentRevision(),
			);
		} catch (reason) {
			for (const stream of pair) stream.clear();
			if (operation !== this.#generation.current || isAbortError(reason))
				return null;
			throw reason;
		}
		const frame = plan.playing ? this.#host.currentFrame() : plan.initialFrame;
		failed = false;
		try {
			pair = await primePreparedPair(pair, {
				frame,
				signal: this.#generation.signal,
				cancel: () => {
					failed = true;
					if (operation === this.#generation.current) this.#generation.renew();
				},
			});
			this.#generation.assert(
				operation,
				plan.snapshot.revision,
				this.#host.currentRevision(),
			);
		} catch (reason) {
			for (const stream of pair) stream.clear();
			if (
				!failed &&
				(operation !== this.#generation.current || isAbortError(reason))
			)
				return null;
			throw reason;
		}
		return {
			streams: pair,
			frame: plan.playing ? this.#host.currentFrame() : frame,
		};
	}

	#streamPlan(plan: PreparationPlan) {
		return {
			frame: plan.initialFrame,
			expectedFrames: snapshotFrames(plan.snapshot),
			signal: this.#generation.signal,
			budget: this.#budget,
		};
	}
}
