import { AudioEngineError } from "./errors";

export const STREAM_BUFFER_BUDGET = 16;

export type BudgetParticipant = Readonly<{ evictOne: () => boolean }>;
export type BufferReservation = Readonly<{
	commit: (buffer: AudioBuffer) => void;
	cancel: () => void;
}>;

export class StreamBufferBudget {
	readonly #buffers = new Set<AudioBuffer>();
	readonly #participants = new Set<BudgetParticipant>();
	#reservations = 0;

	public add(participant: BudgetParticipant): void {
		this.#participants.add(participant);
	}
	public remove(participant: BudgetParticipant): void {
		this.#participants.delete(participant);
	}

	public reserve(): BufferReservation {
		while (this.#buffers.size + this.#reservations >= STREAM_BUFFER_BUDGET) {
			if (
				![...this.#participants].some((participant) => participant.evictOne())
			) {
				throw new AudioEngineError(
					"aggregate stream buffer budget is fully pinned",
				);
			}
		}
		this.#reservations += 1;
		let active = true;
		return Object.freeze({
			commit: (buffer: AudioBuffer) => {
				if (!active) return;
				active = false;
				this.#reservations -= 1;
				this.#buffers.add(buffer);
			},
			cancel: () => {
				if (!active) return;
				active = false;
				this.#reservations -= 1;
			},
		});
	}

	public release(buffer: AudioBuffer): void {
		this.#buffers.delete(buffer);
	}

	public metrics(): Readonly<{
		uniqueBuffers: number;
		retainedBytes: number;
		reservations: number;
	}> {
		return Object.freeze({
			uniqueBuffers: this.#buffers.size,
			retainedBytes: [...this.#buffers].reduce(
				(total, buffer) => total + buffer.length * buffer.numberOfChannels * 4,
				0,
			),
			reservations: this.#reservations,
		});
	}
}
