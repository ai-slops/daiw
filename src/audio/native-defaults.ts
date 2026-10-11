import type { AudioEngineState, MasterMeter } from "./types";

export const initialNativeAudioState: AudioEngineState = {
	status: "stopped",
	positionBeat: 0,
	loop: null,
	revision: null,
	audition: null,
	loudnessMatching: false,
};

export const silentNativeMeter: MasterMeter = {
	peak: 0,
	rms: 0,
	peakDb: -Infinity,
	rmsDb: -Infinity,
};
