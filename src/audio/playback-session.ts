import { playbackFrame } from "./engine-observation";
import { AudioEngineError, isAbortError } from "./errors";
import { loopFrames, snapshotFrames } from "./playback-frames";
import { CHUNK_FRAMES } from "./server-render";
import { type SourceRecord, SourceScheduler } from "./source-scheduler";
import type { StreamBranch } from "./stream-branch";
import type { LoopRange, Snapshot } from "./types";

const TARGET_CHUNKS = 6;
const START_LEAD_SECONDS = 0.05;

export type PlaybackRequest = Readonly<{
	snapshot: Snapshot;
	positionAt: (startWhen: number) => number;
	loop: LoopRange | null;
	stream: StreamBranch;
	inactiveStreams: readonly StreamBranch[];
	generation: number;
	signal: AbortSignal;
	atBoundary: (startWhen: number) => void;
}>;
type PlaybackPlan = Omit<PlaybackRequest, "positionAt"> &
	Readonly<{ positionFrame: number }>;
export type PlaybackStart = Readonly<{
	startWhen: number;
	positionFrame: number;
}>;

type PendingNativeLoop = Readonly<{
	buffer: AudioBuffer;
	frames: Readonly<{ start: number; end: number }>;
}>;
type PlaybackListeners = Readonly<{
	naturalEnd: (frame: number) => void;
	underrun: (frame: number) => void;
	fault: (fault: AudioEngineError) => void;
	retired: (branch: StreamBranch) => void;
}>;

export class PlaybackSession {
	readonly #context: AudioContext;
	readonly #scheduler: SourceScheduler;
	readonly #listeners: PlaybackListeners;
	#plan: PlaybackPlan | null = null;
	#pendingLoop: PendingNativeLoop | null = null;
	#nativeLooping = false;
	#refilling = false;
	#startWhen = 0;

	public constructor(
		context: AudioContext,
		gain: GainNode,
		listeners: PlaybackListeners,
	) {
		this.#context = context;
		this.#listeners = listeners;
		this.#scheduler = new SourceScheduler(context, gain, {
			ended: (record) => this.#ended(record),
			late: (frame) => this.#listeners.underrun(frame),
			retired: (branch) => this.#listeners.retired(branch),
		});
	}

	public async begin(request: PlaybackRequest): Promise<PlaybackStart | null> {
		const frames = loopFrames(request.snapshot, request.loop);
		const shortLoop =
			frames !== null && frames.end - frames.start <= CHUNK_FRAMES;
		const buffer = shortLoop
			? await request.stream.loopBuffer(
					frames.start,
					frames.end,
					request.signal,
				)
			: null;
		if (request.signal.aborted) {
			if (buffer !== null) request.stream.discard(buffer);
			return null;
		}
		const startWhen = this.#context.currentTime + START_LEAD_SECONDS;
		const positionFrame = request.positionAt(startWhen);
		if (!shortLoop && !request.stream.hasFrame(positionFrame)) return null;
		const plan: PlaybackPlan = { ...request, positionFrame };
		request.atBoundary(startWhen);
		this.#transitionAt(startWhen);
		this.#plan = plan;
		this.#startWhen = startWhen;
		this.#scheduler.reset(plan.positionFrame, startWhen);
		if (shortLoop && buffer !== null && frames !== null)
			this.#startShortLoop(buffer, frames);
		else {
			this.#appendAvailable();
			if (this.#scheduler.size === 0) {
				this.#transitionAt();
				return null;
			}
			void this.#refill();
		}
		this.#primeInactive(plan.positionFrame);
		return { startWhen, positionFrame };
	}

	public clear(): void {
		this.#transitionAt();
	}

	public hasScheduled(branch: StreamBranch): boolean {
		return this.#scheduler.hasBranch(branch);
	}

	#transitionAt(when?: number): void {
		const plan = this.#plan;
		if (plan !== null && this.#pendingLoop !== null)
			plan.stream.discard(this.#pendingLoop.buffer);
		this.#plan = null;
		this.#pendingLoop = null;
		this.#nativeLooping = false;
		this.#refilling = false;
		this.#scheduler.clear(when);
	}

	#startShortLoop(
		buffer: AudioBuffer,
		frames: Readonly<{ start: number; end: number }>,
	): void {
		const plan = this.#requiredPlan();
		if (plan.positionFrame < frames.start) {
			this.#pendingLoop = { buffer, frames };
			this.#appendAvailable();
			this.#schedulePendingLoop();
			if (!this.#nativeLooping) void this.#refill();
			return;
		}
		this.#scheduler.nativeLoop(plan.stream, buffer, {
			frames,
			position: plan.positionFrame,
			generation: plan.generation,
		});
		this.#nativeLooping = true;
	}

	#appendAvailable(): void {
		const plan = this.#requiredPlan();
		const loop = loopFrames(plan.snapshot, plan.loop);
		this.#scheduler.appendAvailable(plan.stream, {
			boundary:
				this.#pendingLoop?.frames.start ??
				loop?.end ??
				snapshotFrames(plan.snapshot),
			loopStart: this.#pendingLoop === null ? (loop?.start ?? null) : null,
			generation: plan.generation,
			target: TARGET_CHUNKS,
		});
	}

	async #refill(): Promise<void> {
		if (this.#refilling || this.#nativeLooping) return;
		this.#refilling = true;
		try {
			const plan = this.#requiredPlan();
			while (!plan.signal.aborted && this.#scheduler.size < TARGET_CHUNKS) {
				if (this.#atFetchBoundary(plan)) break;
				await plan.stream.ensure(
					Math.floor(this.#scheduler.cursor / CHUNK_FRAMES) * CHUNK_FRAMES,
					plan.signal,
				);
				if (plan.signal.aborted) return;
				this.#appendAvailable();
				this.#schedulePendingLoop();
				if (this.#nativeLooping) break;
			}
			this.#primeInactive(this.#logicalFrame());
		} catch (reason) {
			const plan = this.#plan;
			if (!isAbortError(reason) && plan !== null && !plan.signal.aborted) {
				this.#listeners.fault(
					new AudioEngineError(
						reason instanceof Error ? reason.message : "audio refill failed",
					),
				);
			}
		} finally {
			this.#refilling = false;
		}
	}

	#ended(record: SourceRecord): void {
		const plan = this.#plan;
		if (plan === null || record.generation !== plan.generation) return;
		if (record.hasSuccessor) {
			void this.#refill();
			return;
		}
		if (
			plan.loop === null &&
			record.endFrame >= snapshotFrames(plan.snapshot)
		) {
			this.#listeners.naturalEnd(snapshotFrames(plan.snapshot));
			return;
		}
		this.#listeners.underrun(record.nextFrame);
	}

	#schedulePendingLoop(): void {
		const pending = this.#pendingLoop;
		const plan = this.#requiredPlan();
		if (pending === null || this.#scheduler.cursor < pending.frames.start)
			return;
		this.#scheduler.nativeLoop(plan.stream, pending.buffer, {
			frames: pending.frames,
			position: pending.frames.start,
			generation: plan.generation,
		});
		this.#pendingLoop = null;
		this.#nativeLooping = true;
	}

	#primeInactive(frame: number): void {
		const plan = this.#requiredPlan();
		for (const stream of plan.inactiveStreams) {
			void stream.prime(frame, plan.signal).catch((reason: unknown) => {
				if (isAbortError(reason) || plan.signal.aborted) return;
				this.#listeners.fault(
					new AudioEngineError(
						reason instanceof Error
							? reason.message
							: "inactive A/B prefetch failed",
					),
				);
			});
		}
	}

	#atFetchBoundary(plan: PlaybackPlan): boolean {
		const pendingStart = this.#pendingLoop?.frames.start;
		if (pendingStart !== undefined)
			return this.#scheduler.cursor >= pendingStart;
		return (
			plan.loop === null &&
			this.#scheduler.cursor >= snapshotFrames(plan.snapshot)
		);
	}

	#requiredPlan(): PlaybackPlan {
		if (this.#plan === null)
			throw new AudioEngineError("playback session is not active");
		return this.#plan;
	}

	#logicalFrame(): number {
		const plan = this.#requiredPlan();
		return playbackFrame({
			snapshot: plan.snapshot,
			anchorFrame: plan.positionFrame,
			anchorWhen: this.#startWhen,
			currentTime: this.#context.currentTime,
			loop: plan.loop,
		});
	}
}
