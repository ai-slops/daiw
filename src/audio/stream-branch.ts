import { AudioEngineError } from "./errors";
import { assembleLoopBuffer, type LoopChunk } from "./loop-buffer";
import {
	CHUNK_FRAMES,
	fetchPcmChunk,
	type RenderDescriptor,
} from "./server-render";
import { type BudgetParticipant, StreamBufferBudget } from "./stream-budget";

const MAX_UNIQUE_BUFFERS = 8;

export class StreamBranch {
	readonly #context: Pick<AudioContext, "createBuffer">;
	readonly #descriptor: RenderDescriptor;
	readonly #cache = new Map<number, LoopChunk>();
	readonly #scheduled = new Map<AudioBuffer, number>();
	readonly #transient = new Set<AudioBuffer>();
	readonly #pinned = new Set<AudioBuffer>();
	readonly #budget: StreamBufferBudget;
	readonly #participant: BudgetParticipant;
	#inflight: Promise<void> = Promise.resolve();
	#inflightCount = 0;
	#anchor = 0;

	public constructor(
		context: Pick<AudioContext, "createBuffer">,
		descriptor: RenderDescriptor,
		budget = new StreamBufferBudget(),
	) {
		this.#context = context;
		this.#descriptor = descriptor;
		this.#budget = budget;
		this.#participant = Object.freeze({ evictOne: () => this.#evictOne() });
		this.#budget.add(this.#participant);
	}

	public get descriptor(): RenderDescriptor {
		return this.#descriptor;
	}

	public hasFrame(frame: number): boolean {
		return (
			frame < this.#descriptor.info.frames &&
			this.#cache.has(this.#chunkStart(frame))
		);
	}

	public chunkAt(frame: number): LoopChunk | null {
		return this.#cache.get(this.#chunkStart(frame)) ?? null;
	}

	public async prime(frame: number, signal: AbortSignal): Promise<void> {
		const start = this.#chunkStart(frame);
		await this.ensure(start, signal);
		const next = start + CHUNK_FRAMES;
		if (next < this.#descriptor.info.frames) await this.ensure(next, signal);
	}

	public async ensure(start: number, signal: AbortSignal): Promise<void> {
		if (start >= this.#descriptor.info.frames || this.#cache.has(start)) return;
		const task = this.#inflight.then(async () => {
			if (this.#cache.has(start) || signal.aborted) return;
			this.#inflightCount = 1;
			try {
				this.#reserveSlot();
				const end = Math.min(
					start + CHUNK_FRAMES,
					this.#descriptor.info.frames,
				);
				const reservation = this.#budget.reserve();
				let buffer: AudioBuffer;
				try {
					buffer = await fetchPcmChunk(this.#context, this.#descriptor, {
						startFrame: start,
						endFrame: end,
						signal,
					});
				} catch (reason) {
					reservation.cancel();
					throw reason;
				}
				reservation.commit(buffer);
				if (signal.aborted) {
					this.#budget.release(buffer);
					return;
				}
				this.#anchor = start;
				this.#cache.set(start, Object.freeze({ start, end, buffer }));
			} finally {
				this.#inflightCount = 0;
			}
		});
		this.#inflight = task.catch(() => undefined);
		await task;
	}

	public async loopBuffer(
		start: number,
		end: number,
		signal: AbortSignal,
	): Promise<AudioBuffer> {
		const first = this.#chunkStart(start);
		const last = this.#chunkStart(end - 1);
		await this.ensure(first, signal);
		if (last !== first) await this.ensure(last, signal);
		const firstChunk = this.chunkAt(start);
		const lastChunk = this.chunkAt(end - 1);
		if (firstChunk === null || lastChunk === null)
			throw new RangeError("loop PCM is unavailable");
		this.#pinned.add(firstChunk.buffer);
		this.#pinned.add(lastChunk.buffer);
		try {
			this.#reserveSlot();
			const reservation = this.#budget.reserve();
			let output: AudioBuffer;
			try {
				output = this.#context.createBuffer(2, end - start, 48_000);
			} catch (reason) {
				reservation.cancel();
				throw reason;
			}
			reservation.commit(output);
			try {
				assembleLoopBuffer(output, { start, end }, (frame) =>
					this.chunkAt(frame),
				);
				this.#transient.add(output);
				return output;
			} catch (reason) {
				this.#budget.release(output);
				throw reason;
			}
		} finally {
			this.#pinned.clear();
		}
	}

	public retain(buffer: AudioBuffer): void {
		this.#transient.delete(buffer);
		this.#scheduled.set(buffer, (this.#scheduled.get(buffer) ?? 0) + 1);
	}

	public release(buffer: AudioBuffer): void {
		const count = this.#scheduled.get(buffer);
		if (count === undefined) return;
		if (count === 1) this.#scheduled.delete(buffer);
		else this.#scheduled.set(buffer, count - 1);
		this.#releaseIfUnused(buffer);
		this.#trimToCap();
	}

	public discard(buffer: AudioBuffer): void {
		this.#transient.delete(buffer);
		this.#releaseIfUnused(buffer);
	}

	public metrics(): Readonly<{
		uniqueBuffers: number;
		cachedChunks: number;
		scheduledBuffers: number;
		inflightRanges: number;
		retainedBytes: number;
	}> {
		const unique = new Set<AudioBuffer>(this.#scheduled.keys());
		for (const chunk of this.#cache.values()) unique.add(chunk.buffer);
		for (const buffer of this.#transient) unique.add(buffer);
		return Object.freeze({
			uniqueBuffers: unique.size,
			cachedChunks: this.#cache.size,
			scheduledBuffers: this.#scheduled.size,
			inflightRanges: this.#inflightCount,
			retainedBytes: [...unique].reduce(
				(total, buffer) => total + buffer.length * buffer.numberOfChannels * 4,
				0,
			),
		});
	}

	public clear(): void {
		const buffers = this.#uniqueBuffers();
		this.#cache.clear();
		this.#scheduled.clear();
		this.#transient.clear();
		this.#pinned.clear();
		for (const buffer of buffers) this.#budget.release(buffer);
		this.#budget.remove(this.#participant);
	}

	#chunkStart(frame: number): number {
		return Math.floor(frame / CHUNK_FRAMES) * CHUNK_FRAMES;
	}

	#reserveSlot(): void {
		while (this.metrics().uniqueBuffers >= MAX_UNIQUE_BUFFERS) {
			const removable = this.#removableChunk();
			if (removable === undefined)
				throw new AudioEngineError("stream buffer cap is fully pinned");
			this.#deleteChunk(removable);
		}
	}

	#trimToCap(): void {
		while (this.metrics().uniqueBuffers > MAX_UNIQUE_BUFFERS) {
			const removable = this.#removableChunk();
			if (removable === undefined) return;
			this.#deleteChunk(removable);
		}
	}

	#removableChunk(): LoopChunk | undefined {
		return [...this.#cache.values()]
			.filter(
				({ buffer }) =>
					!this.#scheduled.has(buffer) && !this.#pinned.has(buffer),
			)
			.sort(
				(left, right) =>
					Math.abs(right.start - this.#anchor) -
					Math.abs(left.start - this.#anchor),
			)[0];
	}

	#evictOne(): boolean {
		const removable = this.#removableChunk();
		if (removable === undefined) return false;
		this.#deleteChunk(removable);
		return true;
	}

	#deleteChunk(chunk: LoopChunk): void {
		this.#cache.delete(chunk.start);
		this.#releaseIfUnused(chunk.buffer);
	}

	#releaseIfUnused(buffer: AudioBuffer): void {
		const cached = [...this.#cache.values()].some(
			(chunk) => chunk.buffer === buffer,
		);
		if (
			!cached &&
			!this.#scheduled.has(buffer) &&
			!this.#transient.has(buffer) &&
			!this.#pinned.has(buffer)
		) {
			this.#budget.release(buffer);
		}
	}

	#uniqueBuffers(): Set<AudioBuffer> {
		const unique = new Set<AudioBuffer>(this.#scheduled.keys());
		for (const chunk of this.#cache.values()) unique.add(chunk.buffer);
		for (const buffer of this.#transient) unique.add(buffer);
		return unique;
	}
}

export const STREAM_BUFFER_CAP = MAX_UNIQUE_BUFFERS;
