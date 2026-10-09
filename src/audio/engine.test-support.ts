import { vi } from "vitest";
import { DaiwAudioEngine } from "./engine";

export class FakeAnalyser {
	public fftSize = 32;
	public smoothingTimeConstant = 0;
	public connect(): void {}
	public disconnect(): void {}
	public getFloatTimeDomainData(): void {}
}

export class FakeAudioParam {
	public value = 1;
	public readonly automation: Array<Readonly<{ value: number; time: number }>> =
		[];
	public readonly cancellations: number[] = [];
	public setValueAtTime(value: number, time: number): this {
		this.automation.push({ value, time });
		return this;
	}
	public cancelScheduledValues(time: number): this {
		this.cancellations.push(time);
		const retained = this.automation.filter((event) => event.time < time);
		this.automation.splice(0, this.automation.length, ...retained);
		return this;
	}
	public advance(time: number): void {
		for (const event of this.automation)
			if (event.time <= time) this.value = event.value;
	}
}

export class FakeGain {
	public readonly gain = new FakeAudioParam();
	public connect(node: FakeAnalyser): FakeAnalyser {
		return node;
	}
	public disconnect(): void {}
}

export class FakeBuffer {
	public readonly duration: number;
	public readonly numberOfChannels = 2;
	readonly #channels: Float32Array<ArrayBuffer>[];
	public constructor(
		public readonly length: number,
		public readonly sampleRate = 48_000,
	) {
		this.duration = length / sampleRate;
		this.#channels = [new Float32Array(length), new Float32Array(length)];
	}
	public getChannelData(channel: number): Float32Array<ArrayBuffer> {
		return this.#channels[channel] ?? new Float32Array();
	}
	public copyFromChannel(
		destination: Float32Array<ArrayBuffer>,
		channel: number,
		offset = 0,
	): void {
		destination.set(
			this.getChannelData(channel).subarray(
				offset,
				offset + destination.length,
			),
		);
	}
	public copyToChannel(
		source: Float32Array<ArrayBuffer>,
		channel: number,
		offset = 0,
	): void {
		this.#channels[channel]?.set(source, offset);
	}
}

export class FakeSource {
	public buffer: FakeBuffer | null = null;
	public loop = false;
	public loopStart = 0;
	public loopEnd = 0;
	public onended: (() => void) | null = null;
	public readonly start =
		vi.fn<(when: number, offset?: number, duration?: number) => void>();
	public readonly connect = vi.fn<(node: FakeGain) => FakeGain>((node) => node);
	public readonly disconnect = vi.fn<() => void>();
	public readonly stop = vi.fn<(_when?: number) => void>();
	public end(): void {
		this.onended?.();
	}
}

export class FakeAudioContext {
	public currentTime = 0;
	public advanceOnCreateSource = 0;
	public readonly destination = {};
	public readonly sources: FakeSource[] = [];
	public readonly gain = new FakeGain();
	public resumed = 0;
	public closed = 0;
	public createAnalyser(): FakeAnalyser {
		return new FakeAnalyser();
	}
	public createBufferSource(): FakeSource {
		this.currentTime += this.advanceOnCreateSource;
		const source = new FakeSource();
		this.sources.push(source);
		return source;
	}
	public createGain(): FakeGain {
		return this.gain;
	}
	public createBuffer(
		_channels: number,
		frames: number,
		rate: number,
	): FakeBuffer {
		return new FakeBuffer(frames, rate);
	}
	public resume(): Promise<void> {
		this.resumed += 1;
		return Promise.resolve();
	}
	public close(): Promise<void> {
		this.closed += 1;
		return Promise.resolve();
	}
}

export function createEngine(context: FakeAudioContext): DaiwAudioEngine {
	vi.stubGlobal("AudioContext", function AudioContextFactory() {
		return context;
	});
	return new DaiwAudioEngine();
}

export function snapshot(length = 8, revision = "revision-1") {
	return { revision, tempo: 60, length, tracks: [], notes: [], regions: [] };
}

export async function flush(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}
