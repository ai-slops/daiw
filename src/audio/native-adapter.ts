import type {
	NativeDevice,
	NativeWorkerEvent,
	NativeWorkerState,
} from "../native-audio/bridge";
import { beatAtFrame } from "./playback-frames";
import type { AudioEngineState, Snapshot } from "./types";

export type NativeEventResult = Readonly<{
	state: AudioEngineState;
	fault: string | null;
}>;

export function nativeAudioState(
	snapshot: Snapshot,
	input: NativeWorkerState,
): AudioEngineState {
	return {
		status: input.status,
		positionBeat: beatAtFrame(snapshot, input.positionFrame),
		loop:
			input.loop === null
				? null
				: {
						start: beatAtFrame(snapshot, input.loop.start),
						end: beatAtFrame(snapshot, input.loop.end),
					},
		revision: snapshot.revision,
		audition: null,
		loudnessMatching: false,
	};
}

export function selectNativeDevice(
	current: string | undefined,
	devices: readonly NativeDevice[],
): string | undefined {
	if (
		current !== undefined &&
		devices.some((device) => device.token === current && device.supported)
	)
		return current;
	return (
		devices.find((device) => device.isDefault && device.supported)?.token ??
		devices.find((device) => device.supported)?.token
	);
}

type NativeCompletionKey = Readonly<{
	backend: "browser" | "native";
	sessionToken: string;
	projectId: string;
	revision: string;
}>;

export function shouldAcceptNativeCompletion(
	current: NativeCompletionKey,
	completed: NativeCompletionKey,
): boolean {
	return (
		current.backend === "native" &&
		current.sessionToken === completed.sessionToken &&
		current.projectId === completed.projectId &&
		current.revision === completed.revision
	);
}

export function nativeAudioEventState(
	snapshot: Snapshot,
	current: NativeCompletionKey,
	event: NativeWorkerEvent,
): AudioEngineState | null {
	if (!("state" in event)) return null;
	return shouldAcceptNativeCompletion(current, {
		backend: "native",
		sessionToken: event.sessionToken,
		projectId: event.projectId,
		revision: event.expectedRevision,
	})
		? nativeAudioState(snapshot, event.state)
		: null;
}

export function nativeAudioEvent(
	snapshot: Snapshot,
	current: NativeCompletionKey,
	event: NativeWorkerEvent,
): NativeEventResult | null {
	if (
		!shouldAcceptNativeCompletion(current, {
			backend: "native",
			sessionToken: event.sessionToken,
			projectId: event.projectId,
			revision: event.expectedRevision,
		})
	)
		return null;
	if ("state" in event)
		return { state: nativeAudioState(snapshot, event.state), fault: null };
	return {
		state: {
			status: "faulted",
			positionBeat: 0,
			loop: null,
			revision: snapshot.revision,
			audition: null,
			loudnessMatching: false,
		},
		fault: event.fault.message,
	};
}
