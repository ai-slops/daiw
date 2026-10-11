import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AudioEngineState, LoopRange, Snapshot } from "../audio";
import { nativeAudioState } from "../audio/native-adapter";
import {
	initialNativeAudioState,
	silentNativeMeter,
} from "../audio/native-defaults";
import { frameAtBeat, loopFrames } from "../audio/playback-frames";
import { fourBarRange } from "../components/transport-timing";
import { SnapshotResultSchema } from "../domain/api";
import type { Project } from "../domain/project";
import { rpc } from "../lib/api-client";
import {
	type NativeWorkerState,
	nativeBridge,
	nativeBridgeAvailable,
} from "../native-audio/bridge";
import {
	type NativePreparation,
	nativeErrorMessage,
	useNativeAudioEvents,
} from "./use-native-audio-events";
import { useNativeDevices } from "./use-native-devices";

export const useNativeAudio = (project: Project, enabled: boolean) => {
	const [state, setState] = useState<AudioEngineState>(initialNativeAudioState);
	const [busy, setBusy] = useState(false);
	const snapshotRef = useRef<Snapshot | null>(null);
	const sessionRef = useRef<string | null>(null);
	const generationRef = useRef(0);
	const preparingRef = useRef<NativePreparation | null>(null);
	const bridge = useMemo(() => nativeBridge(), []);
	const {
		available,
		devices,
		deviceToken,
		deviceLoading,
		error,
		setError,
		setDeviceToken,
		refreshDevices,
	} = useNativeDevices(bridge);

	const release = useCallback(async () => {
		const generation = generationRef.current + 1;
		generationRef.current = generation;
		preparingRef.current = null;
		const session = sessionRef.current;
		sessionRef.current = null;
		try {
			if (session !== null && bridge !== undefined)
				await bridge.release(session);
			if (generationRef.current === generation)
				setState(initialNativeAudioState);
			return true;
		} catch (reason) {
			if (generationRef.current !== generation) return true;
			sessionRef.current = session;
			setError(nativeErrorMessage(reason, "Native audio release failed."));
			return false;
		}
	}, [bridge, setError]);

	useNativeAudioEvents({
		bridge,
		enabled,
		projectId: project.id,
		revision: project.revision,
		snapshotRef,
		sessionRef,
		generationRef,
		preparingRef,
		setState,
		setError,
	});
	useEffect(() => {
		let active = true;
		if (enabled)
			void rpc(
				{ projectId: project.id, method: "project.snapshot" },
				SnapshotResultSchema,
			)
				.then(({ result }) => {
					if (active && result.snapshot.revision === project.revision)
						snapshotRef.current = result.snapshot;
				})
				.catch((reason: unknown) => {
					if (active)
						setError(nativeErrorMessage(reason, "Native snapshot failed."));
				});
		return () => {
			active = false;
			void release();
		};
	}, [enabled, project.id, project.revision, release, setError]);

	const prepare = useCallback((): Promise<string | null> => {
		const identity = `${sessionRef.current ?? ""}:${project.id}:${project.revision}:${deviceToken ?? ""}`;
		const key = `${generationRef.current}:${identity}`;
		if (preparingRef.current?.key === key) return preparingRef.current.promise;
		const preparation = (async () => {
			const snapshot = snapshotRef.current;
			if (
				bridge === undefined ||
				deviceToken === undefined ||
				snapshot?.revision !== project.revision
			) {
				setError(
					deviceToken === undefined
						? "No supported native output device is available."
						: "Native audio snapshot is still loading.",
				);
				return null;
			}
			setBusy(true);
			setError(undefined);
			const generation = generationRef.current + 1;
			generationRef.current = generation;
			try {
				const prior = sessionRef.current;
				if (prior !== null) {
					sessionRef.current = null;
					await bridge.release(prior);
				}
				const prepared = await bridge.prepare({
					projectId: project.id,
					expectedRevision: project.revision,
					deviceToken,
				});
				if (generationRef.current !== generation || !enabled) {
					await bridge.release(prepared.sessionToken);
					return null;
				}
				sessionRef.current = prepared.sessionToken;
				return prepared.sessionToken;
			} catch (reason) {
				if (generationRef.current === generation)
					setError(
						nativeErrorMessage(reason, "Native audio preparation failed."),
					);
				return null;
			} finally {
				if (generationRef.current === generation) setBusy(false);
			}
		})();
		preparingRef.current = {
			key: `${generationRef.current}:${identity}`,
			promise: preparation,
		};
		const clear = () => {
			if (preparingRef.current?.promise === preparation)
				preparingRef.current = null;
		};
		void preparation.then(clear, clear);
		return preparation;
	}, [bridge, deviceToken, enabled, project.id, project.revision, setError]);

	const withSession = useCallback(
		async (operation: (session: string) => Promise<NativeWorkerState>) => {
			const session = sessionRef.current ?? (await prepare());
			if (session === null) return;
			const generation = generationRef.current;
			try {
				const worker = await operation(session);
				const snapshot = snapshotRef.current;
				if (
					snapshot !== null &&
					generationRef.current === generation &&
					enabled &&
					sessionRef.current === session &&
					snapshot.revision === project.revision
				)
					setState(nativeAudioState(snapshot, worker));
			} catch (reason) {
				const snapshot = snapshotRef.current;
				if (
					generationRef.current === generation &&
					sessionRef.current === session &&
					enabled &&
					snapshot?.revision === project.revision
				)
					setError(nativeErrorMessage(reason, "Native audio command failed."));
			}
		},
		[enabled, prepare, project.revision, setError],
	);

	const setLoop = useCallback(
		(loop: LoopRange | null) => {
			const snapshot = snapshotRef.current;
			if (snapshot !== null)
				void withSession(
					(session) =>
						bridge?.setLoop(session, loopFrames(snapshot, loop)) ??
						Promise.reject(new TypeError("Native bridge unavailable")),
				);
		},
		[bridge, withSession],
	);

	return {
		state,
		meter: silentNativeMeter,
		busy,
		error,
		devices,
		deviceToken,
		deviceLoading,
		available: nativeBridgeAvailable() && available,
		setDeviceToken: (next: string) => {
			void release().then((released) => {
				if (released) setDeviceToken(next);
			});
		},
		refreshDevices,
		toggle: () =>
			withSession(
				(session) =>
					bridge?.command(
						state.status === "playing" ? "pause" : "play",
						session,
					) ?? Promise.reject(new TypeError("Native bridge unavailable")),
			),
		stop: () => {
			void withSession(
				(session) =>
					bridge?.command("stop", session) ??
					Promise.reject(new TypeError("Native bridge unavailable")),
			);
		},
		seek: (beat: number) => {
			const snapshot = snapshotRef.current;
			if (snapshot !== null)
				void withSession(
					(session) =>
						bridge?.seek(session, frameAtBeat(snapshot, beat)) ??
						Promise.reject(new TypeError("Native bridge unavailable")),
				);
		},
		toggleLoop: () => {
			const snapshot = snapshotRef.current;
			if (snapshot !== null)
				setLoop(
					state.loop === null
						? fourBarRange(
								state.positionBeat,
								snapshot.length,
								project.meterMap ?? project.meter,
							)
						: null,
				);
		},
		release,
	};
};
