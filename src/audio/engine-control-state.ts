import { playbackFrame } from "./engine-observation";
import { AudioEngineError } from "./errors";
import { beatAtFrame } from "./playback-frames";
import type {
	AudioEngineState,
	AuditionSide,
	LoopRange,
	Snapshot,
	TransportStatus,
} from "./types";

export type EngineControlState = {
	snapshot: Snapshot | null;
	status: TransportStatus;
	positionFrame: number;
	anchorFrame: number;
	anchorWhen: number;
	loop: LoopRange | null;
	audition: AuditionSide | null;
	loudnessMatching: boolean;
};

export function createEngineControlState(): EngineControlState {
	return {
		snapshot: null,
		status: "stopped",
		positionFrame: 0,
		anchorFrame: 0,
		anchorWhen: 0,
		loop: null,
		audition: null,
		loudnessMatching: false,
	};
}

export function requireSnapshot(state: EngineControlState): Snapshot {
	if (state.snapshot === null)
		throw new AudioEngineError("setSnapshot must be called first");
	return state.snapshot;
}

export function currentFrame(
	state: EngineControlState,
	currentTime: number,
): number {
	return playbackFrame({
		snapshot: requireSnapshot(state),
		anchorFrame: state.anchorFrame,
		anchorWhen: state.anchorWhen,
		currentTime,
		loop: state.loop,
	});
}

export function positionBeats(
	state: EngineControlState,
	currentTime: number,
): number {
	if (state.snapshot === null) return 0;
	const frame =
		state.status === "playing"
			? currentFrame(state, currentTime)
			: state.positionFrame;
	return beatAtFrame(state.snapshot, frame);
}

export function engineState(
	state: EngineControlState,
	currentTime: number,
): AudioEngineState {
	return {
		status: state.status,
		positionBeat: positionBeats(state, currentTime),
		loop: state.loop,
		revision: state.snapshot?.revision ?? null,
		audition: state.audition,
		loudnessMatching: state.loudnessMatching,
	};
}
