import { type RefObject, useCallback, useEffect } from "react";
import { nativeAudioEvent } from "../audio/native-adapter";
import type { AudioEngineState, Snapshot } from "../audio/types";
import type { NativeBridge, NativeWorkerEvent } from "../native-audio/bridge";

export type NativePreparation = Readonly<{
	key: string;
	promise: Promise<string | null>;
}>;

export const nativeErrorMessage = (
	reason: unknown,
	fallback: string,
): string => (reason instanceof Error ? reason.message : fallback);

type NativeEventContext = Readonly<{
	bridge: NativeBridge | undefined;
	enabled: boolean;
	projectId: string;
	revision: string;
	snapshotRef: RefObject<Snapshot | null>;
	sessionRef: RefObject<string | null>;
	generationRef: RefObject<number>;
	preparingRef: RefObject<NativePreparation | null>;
	setState: (state: AudioEngineState) => void;
	setError: (error: string | undefined) => void;
}>;

export const useNativeAudioEvents = (context: NativeEventContext): void => {
	const applyEvent = useCallback(
		(event: NativeWorkerEvent) => {
			const snapshot = context.snapshotRef.current;
			if (snapshot === null) return;
			const next = nativeAudioEvent(
				snapshot,
				{
					backend: context.enabled ? "native" : "browser",
					sessionToken: context.sessionRef.current ?? "",
					projectId: context.projectId,
					revision: context.revision,
				},
				event,
			);
			if (next === null) return;
			context.setState(next.state);
			if (next.fault !== null) {
				context.sessionRef.current = null;
				context.generationRef.current += 1;
				context.preparingRef.current = null;
				context.setError(next.fault);
			}
		},
		[context],
	);
	useEffect(
		() =>
			context.bridge?.onState(applyEvent, (reason) =>
				context.setError(`Invalid native audio state: ${reason.message}`),
			),
		[applyEvent, context],
	);
};
