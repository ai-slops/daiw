export class AudioEngineError extends Error {
	public readonly name: string = "AudioEngineError";
	public constructor(message: string) {
		super(message);
	}
}

export class UnderrunError extends AudioEngineError {
	public readonly name = "UnderrunError";
	public constructor(public readonly missingFrame: number) {
		super(`audio underrun at frame ${missingFrame}`);
	}
}

export type AudioEngineFault = UnderrunError | AudioEngineError;

export function isAbortError(reason: unknown): boolean {
	return reason instanceof Error && reason.name === "AbortError";
}
