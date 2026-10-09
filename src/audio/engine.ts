import { AuditionPreparer } from "./audition-preparer";
import { BoundaryGain } from "./boundary-gain";
import {
	createEngineControlState,
	currentFrame,
	engineState,
	requireSnapshot,
} from "./engine-control-state";
import { EngineObservers } from "./engine-observers";
import { EnginePlaybackRuntime } from "./engine-playback-runtime";
import { EngineStreamSet } from "./engine-stream-set";
import {
	AudioEngineError,
	type AudioEngineFault,
	isAbortError,
} from "./errors";
import {
	frameAtBeat,
	loopFrames,
	normalizedBeat,
	normalizeFrame,
	snapshotFrames,
} from "./playback-frames";
import { validateRenderedLoop } from "./rendered-loop";
import { TransportIntent } from "./transport-intent";
import type {
	AudioEngineState,
	AuditionSide,
	LoopRange,
	MasterMeter,
	Snapshot,
	StateListener,
} from "./types";
import { snapshotSchema } from "./types";

export class DaiwAudioEngine {
	readonly #context: AudioContext;
	readonly #streams = new EngineStreamSet();
	readonly #gain: BoundaryGain;
	readonly #preparer: AuditionPreparer;
	readonly #intent: TransportIntent;
	readonly #observers: EngineObservers;
	readonly #state = createEngineControlState();
	readonly #runtime: EnginePlaybackRuntime;

	public constructor(
		context?: AudioContext,
		private readonly ownsContext = context === undefined,
	) {
		this.#context = context ?? new AudioContext();
		this.#observers = new EngineObservers(this.#context);
		this.#gain = new BoundaryGain(this.#context, this.#observers.input);
		this.#preparer = new AuditionPreparer(
			this.#context,
			{
				currentRevision: () => this.#state.snapshot?.revision ?? null,
				currentFrame: () =>
					currentFrame(this.#state, this.#context.currentTime),
			},
			this.#streams.budget,
		);
		this.#intent = new TransportIntent(this.#preparer, this.#gain);
		this.#runtime = new EnginePlaybackRuntime({
			context: this.#context,
			state: this.#state,
			streams: this.#streams,
			gain: this.#gain,
			intent: this.#intent,
			emitState: () => this.#emit(),
			emitFault: (fault) => this.#observers.emitFault(fault),
		});
	}

	public setSnapshot(input: unknown): Snapshot {
		this.stop();
		this.#streams.clear();
		this.#state.audition = null;
		this.#state.loop = null;
		const snapshot = Object.freeze(snapshotSchema.parse(input));
		this.#state.snapshot = snapshot;
		this.#emit();
		return snapshot;
	}

	public async activate(): Promise<void> {
		await this.#context.resume();
	}

	public async prepare(projectId: string): Promise<void> {
		const snapshot = requireSnapshot(this.#state);
		const playing = this.#state.status === "playing";
		const initialFrame = playing
			? currentFrame(this.#state, this.#context.currentTime)
			: this.#state.positionFrame;
		const prepared = await this.#preparer.single(
			{ projectId, expectedRevision: snapshot.revision },
			{ snapshot, initialFrame, playing },
		);
		if (prepared === null) return;
		await this.#runtime.installPrepared(prepared, playing);
	}

	public async prepareAB(projectId: string, proposalId: string): Promise<void> {
		const snapshot = requireSnapshot(this.#state);
		const playing = this.#state.status === "playing";
		const initialFrame = playing
			? currentFrame(this.#state, this.#context.currentTime)
			: this.#state.positionFrame;
		const prepared = await this.#preparer.pair(
			{
				reference: { projectId, expectedRevision: snapshot.revision },
				candidate: {
					projectId,
					expectedRevision: snapshot.revision,
					proposalId,
				},
			},
			{ snapshot, initialFrame, playing },
		);
		if (prepared === null) return;
		await this.#runtime.installPrepared(prepared, playing);
	}

	public async play(): Promise<void> {
		const revision = requireSnapshot(this.#state).revision;
		await this.activate();
		this.#intent.cancelPending();
		if (this.#state.status === "playing") return;
		if (
			this.#state.loop === null &&
			this.#state.positionFrame >= snapshotFrames(requireSnapshot(this.#state))
		)
			return;
		if (this.#state.snapshot?.revision !== revision) return;
		const stream = this.#streams.required(this.#state.audition);
		const operation = this.#intent.renewSystem();
		try {
			await stream.prime(this.#state.positionFrame, this.#intent.signal);
		} catch (reason) {
			if (operation !== this.#intent.current || isAbortError(reason)) return;
			throw reason;
		}
		this.#intent.assert(
			operation,
			revision,
			this.#state.snapshot?.revision ?? null,
		);
		await this.#runtime.begin(operation);
	}

	public pause(): void {
		this.#intent.renewUser();
		if (this.#state.status !== "playing") return;
		this.#state.positionFrame = currentFrame(
			this.#state,
			this.#context.currentTime,
		);
		this.#runtime.clear();
		this.#state.status = "paused";
		this.#emit();
	}

	public stop(): void {
		this.#intent.renewUser();
		this.#runtime.clear();
		this.#state.status = "stopped";
		this.#state.positionFrame = 0;
		this.#emit();
	}

	public seek(beat: number): void {
		const snapshot = requireSnapshot(this.#state);
		const playing = this.#state.status === "playing";
		this.#state.positionFrame = frameAtBeat(
			snapshot,
			normalizedBeat(snapshot, beat, this.#state.loop),
		);
		this.#runtime.restart(playing);
	}

	public setLoop(loop: LoopRange | null): void {
		const snapshot = requireSnapshot(this.#state);
		const playing = this.#state.status === "playing";
		if (playing)
			this.#state.positionFrame = currentFrame(
				this.#state,
				this.#context.currentTime,
			);
		this.#state.loop = validateRenderedLoop(snapshot, loop);
		this.#state.positionFrame = normalizeFrame(
			this.#state.positionFrame,
			loopFrames(snapshot, this.#state.loop),
		);
		this.#runtime.restart(playing);
	}

	public setLoudnessMatching(enabled: boolean): void {
		this.#state.loudnessMatching = enabled;
		this.#gain.set(
			this.#streams.gain(this.#state.audition, this.#state.loudnessMatching),
		);
		this.#emit();
	}

	public selectAB(side: AuditionSide): void {
		const selected = this.#streams.installed.get(side);
		if (selected === undefined)
			throw new AudioEngineError(`audition ${side} is not prepared`);
		const playing = this.#state.status === "playing";
		if (playing)
			this.#state.positionFrame = currentFrame(
				this.#state,
				this.#context.currentTime,
			);
		const operation = this.#intent.renewUser();
		this.#state.audition = side;
		if (!playing)
			this.#gain.set(
				this.#streams.gain(this.#state.audition, this.#state.loudnessMatching),
			);
		if (playing && selected.hasFrame(this.#state.positionFrame))
			this.#runtime.startPrepared(operation);
		else if (playing) {
			this.#runtime.clear();
			this.#state.status = "paused";
			void this.#runtime.refillSelection(selected, operation);
		}
		this.#emit();
	}

	public getPositionBeats(): number {
		return this.getState().positionBeat;
	}

	public getState(): AudioEngineState {
		return engineState(this.#state, this.#context.currentTime);
	}

	public getMeter(): MasterMeter {
		return this.#observers.meter(this.#state.status);
	}

	public getStreamingMetrics() {
		return this.#streams.metrics();
	}

	public subscribe(listener: StateListener): () => void {
		return this.#observers.subscribe(listener, this.getState());
	}

	public subscribeFault(
		listener: (fault: AudioEngineFault) => void,
	): () => void {
		return this.#observers.subscribeFault(listener);
	}

	public async dispose(): Promise<void> {
		this.stop();
		this.#streams.clear();
		this.#observers.dispose();
		this.#gain.dispose();
		if (this.ownsContext) await this.#context.close();
	}

	#emit(): void {
		this.#observers.emit(this.getState());
	}
}
