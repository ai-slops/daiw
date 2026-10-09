import { CHUNK_FRAMES } from "./server-render";
import type { StreamBranch } from "./stream-branch";

export type SourceRecord = {
	readonly source: AudioBufferSourceNode;
	readonly branch: StreamBranch;
	readonly buffer: AudioBuffer;
	readonly endFrame: number;
	readonly nextFrame: number;
	readonly generation: number;
	hasSuccessor: boolean;
};

type EndedListener = (record: SourceRecord) => void;
type LateListener = (frame: number) => void;
type SchedulerListeners = Readonly<{
	ended: EndedListener;
	late: LateListener;
	retired: (branch: StreamBranch) => void;
}>;
type AppendPlan = Readonly<{
	boundary: number;
	loopStart: number | null;
	generation: number;
	target: number;
}>;
type NativeLoopPlan = Readonly<{
	frames: Readonly<{ start: number; end: number }>;
	position: number;
	generation: number;
}>;

export class SourceScheduler {
	readonly #context: AudioContext;
	readonly #gain: GainNode;
	readonly #endedListener: EndedListener;
	readonly #lateListener: LateListener;
	readonly #retiredListener: (branch: StreamBranch) => void;
	readonly #records = new Map<AudioBufferSourceNode, SourceRecord>();
	readonly #retiring = new Set<SourceRecord>();
	#tail: SourceRecord | null = null;
	#cursor = 0;
	#when = 0;

	public constructor(
		context: AudioContext,
		gain: GainNode,
		listeners: SchedulerListeners,
	) {
		this.#context = context;
		this.#gain = gain;
		this.#endedListener = listeners.ended;
		this.#lateListener = listeners.late;
		this.#retiredListener = listeners.retired;
	}

	public get size(): number {
		return this.#records.size;
	}
	public get cursor(): number {
		return this.#cursor;
	}
	public get when(): number {
		return this.#when;
	}

	public hasBranch(branch: StreamBranch): boolean {
		return [...this.#records.values(), ...this.#retiring].some(
			(record) => record.branch === branch,
		);
	}

	public reset(frame: number, when: number): void {
		this.#cursor = frame;
		this.#when = when;
	}

	public appendAvailable(branch: StreamBranch, plan: AppendPlan): void {
		while (this.#records.size < plan.target) {
			const chunk = branch.chunkAt(this.#cursor);
			if (chunk === null) return;
			const end = Math.min(chunk.end, plan.boundary);
			if (end <= this.#cursor) return;
			const next =
				plan.loopStart !== null && end === plan.boundary ? plan.loopStart : end;
			if (this.#when < this.#context.currentTime) {
				this.#lateListener(this.#cursor);
				return;
			}
			const source = this.#context.createBufferSource();
			source.buffer = chunk.buffer;
			source.connect(this.#gain);
			const record: SourceRecord = {
				source,
				branch,
				buffer: chunk.buffer,
				endFrame: end,
				nextFrame: next,
				generation: plan.generation,
				hasSuccessor: false,
			};
			if (this.#tail !== null) this.#tail.hasSuccessor = true;
			this.#tail = record;
			this.#records.set(source, record);
			branch.retain(chunk.buffer);
			source.onended = () => this.#handleEnded(record);
			const duration = (end - this.#cursor) / 48_000;
			source.start(
				this.#when,
				(this.#cursor % CHUNK_FRAMES) / 48_000,
				duration,
			);
			this.#cursor = next;
			this.#when += duration;
		}
	}

	public nativeLoop(
		branch: StreamBranch,
		buffer: AudioBuffer,
		plan: NativeLoopPlan,
	): void {
		const source = this.#context.createBufferSource();
		source.buffer = buffer;
		source.loop = true;
		source.loopStart = 0;
		source.loopEnd = (plan.frames.end - plan.frames.start) / 48_000;
		source.connect(this.#gain);
		branch.retain(buffer);
		if (this.#tail !== null) this.#tail.hasSuccessor = true;
		source.start(this.#when, (plan.position - plan.frames.start) / 48_000);
		const record: SourceRecord = {
			source,
			branch,
			buffer,
			endFrame: plan.frames.end,
			nextFrame: plan.frames.start,
			generation: plan.generation,
			hasSuccessor: true,
		};
		this.#records.set(source, record);
		this.#tail = record;
	}

	public clear(when?: number): void {
		if (when !== undefined && when > this.#context.currentTime) {
			for (const record of this.#records.values()) {
				record.source.onended = () => this.#finishRetirement(record);
				record.source.stop(when);
				this.#retiring.add(record);
			}
			this.#records.clear();
			this.#tail = null;
			return;
		}
		const branches = new Set<StreamBranch>();
		for (const record of this.#records.values()) {
			branches.add(record.branch);
			record.source.onended = null;
			record.source.stop(when);
			record.source.disconnect();
			record.branch.release(record.buffer);
		}
		for (const record of this.#retiring) {
			branches.add(record.branch);
			record.source.onended = null;
			record.source.stop();
			record.source.disconnect();
			record.branch.release(record.buffer);
		}
		this.#records.clear();
		this.#retiring.clear();
		this.#tail = null;
		for (const branch of branches) this.#retiredListener(branch);
	}

	#handleEnded(record: SourceRecord): void {
		if (!this.#records.delete(record.source)) return;
		record.branch.release(record.buffer);
		this.#endedListener(record);
	}

	#finishRetirement(record: SourceRecord): void {
		if (!this.#retiring.delete(record)) return;
		record.source.disconnect();
		record.branch.release(record.buffer);
		if (!this.hasBranch(record.branch)) this.#retiredListener(record.branch);
	}
}
