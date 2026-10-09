export class BoundaryGain {
	public readonly node: GainNode;
	readonly #context: AudioContext;

	public constructor(context: AudioContext, output: AudioNode) {
		this.#context = context;
		this.node = context.createGain();
		this.node.connect(output);
	}

	public set(value: number): void {
		this.cancel();
		this.node.gain.value = value;
	}

	public schedule(value: number, when: number): void {
		this.cancel();
		this.node.gain.setValueAtTime(value, when);
	}

	public cancel(): void {
		this.node.gain.cancelScheduledValues(this.#context.currentTime);
	}
	public dispose(): void {
		this.cancel();
		this.node.disconnect();
	}
}
