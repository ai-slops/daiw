import type { PreparedAuditions } from "./audition-preparer";
import type { BoundaryGain } from "./boundary-gain";
import {
	type EngineControlState,
	requireSnapshot,
} from "./engine-control-state";
import { playbackFrame } from "./engine-observation";
import type { EngineStreamSet } from "./engine-stream-set";
import {
	AudioEngineError,
	type AudioEngineFault,
	isAbortError,
	UnderrunError,
} from "./errors";
import { PlaybackSession } from "./playback-session";
import type { StreamBranch } from "./stream-branch";
import type { TransportIntent } from "./transport-intent";

type EnginePlaybackRuntimeConfig = Readonly<{
	context: AudioContext;
	state: EngineControlState;
	streams: EngineStreamSet;
	gain: BoundaryGain;
	intent: TransportIntent;
	emitState: () => void;
	emitFault: (fault: AudioEngineFault) => void;
}>;

export class EnginePlaybackRuntime {
	readonly #playback: PlaybackSession;

	public constructor(private readonly config: EnginePlaybackRuntimeConfig) {
		this.#playback = new PlaybackSession(config.context, config.gain.node, {
			naturalEnd: (frame) => this.#naturalEnd(frame),
			underrun: (frame) => this.#underrun(frame),
			fault: config.emitFault,
			retired: (stream) => config.streams.releaseRetired(stream),
		});
	}

	public clear(): void {
		this.#playback.clear();
	}

	public async begin(
		operation: number,
		advanceToBoundary = false,
	): Promise<void> {
		const { context, gain, intent, state, streams } = this.config;
		const snapshot = requireSnapshot(state);
		const stream = streams.required(state.audition);
		const inactiveStreams = [...streams.installed.entries()]
			.filter(([side]) => side !== state.audition)
			.map(([, inactive]) => inactive);
		const targetGain = streams.gain(state.audition, state.loudnessMatching);
		const capturedFrame = state.positionFrame;
		const capturedWhen = context.currentTime;
		const start = await this.#playback.begin({
			snapshot,
			positionAt: (startWhen) =>
				advanceToBoundary
					? playbackFrame({
							snapshot,
							anchorFrame: capturedFrame,
							anchorWhen: capturedWhen,
							currentTime: startWhen,
							loop: state.loop,
						})
					: capturedFrame,
			loop: state.loop,
			stream,
			inactiveStreams,
			generation: operation,
			signal: intent.signal,
			atBoundary: (startWhen) => gain.schedule(targetGain, startWhen),
		});
		if (start === null || operation !== intent.current) return;
		state.status = "playing";
		state.positionFrame = start.positionFrame;
		state.anchorFrame = start.positionFrame;
		state.anchorWhen = start.startWhen;
		this.config.emitState();
	}

	public restart(playing: boolean): void {
		const { intent, state, streams } = this.config;
		const operation = intent.renewUser();
		this.#playback.clear();
		state.status = playing ? "paused" : state.status;
		if (playing)
			void this.refillSelection(streams.required(state.audition), operation);
		this.config.emitState();
	}

	public async refillSelection(
		stream: StreamBranch,
		operation: number,
	): Promise<void> {
		const { intent, state } = this.config;
		try {
			await stream.prime(state.positionFrame, intent.signal);
			if (operation === intent.current) await this.begin(operation);
		} catch (reason) {
			if (!isAbortError(reason) && operation === intent.current) {
				this.config.emitFault(
					new AudioEngineError(
						reason instanceof Error ? reason.message : "audio refill failed",
					),
				);
			}
		}
	}

	public startPrepared(operation: number): void {
		void this.begin(operation, true).catch((reason: unknown) => {
			if (isAbortError(reason) || operation !== this.config.intent.current)
				return;
			this.#playback.clear();
			this.config.state.status = "paused";
			this.config.emitState();
			this.config.emitFault(
				new AudioEngineError(
					reason instanceof Error
						? reason.message
						: "playback scheduling failed",
				),
			);
		});
	}

	public async installPrepared(
		prepared: PreparedAuditions,
		playing: boolean,
	): Promise<void> {
		const { intent, state, streams } = this.config;
		const obsolete = streams.replace(prepared);
		if (!playing) this.#playback.clear();
		state.audition = "a";
		state.positionFrame = prepared.frame;
		const operation = intent.renewSystem();
		try {
			if (playing) await this.begin(operation, true);
			else this.config.emitState();
		} catch (reason) {
			this.#playback.clear();
			state.status = "paused";
			throw reason;
		} finally {
			streams.retainObsolete(obsolete, (stream) =>
				this.#playback.hasScheduled(stream),
			);
		}
	}

	#naturalEnd(frame: number): void {
		const { intent, state } = this.config;
		intent.renewUser();
		this.#playback.clear();
		state.positionFrame = frame;
		state.status = "stopped";
		this.config.emitState();
	}

	#underrun(frame: number): void {
		const { intent, state } = this.config;
		if (state.status !== "playing") return;
		intent.renewUser();
		state.positionFrame = frame;
		this.#playback.clear();
		state.status = "paused";
		this.config.emitState();
		this.config.emitFault(new UnderrunError(frame));
	}
}
