import { useCallback, useEffect, useRef, useState } from "react";
import type {
	AudioEngineState,
	AuditionSide,
	MasterMeter,
	Snapshot,
} from "../audio";
import { DaiwAudioEngine } from "../audio";
import { fourBarRange } from "../components/transport-timing";
import { SnapshotResultSchema } from "../domain/api";
import type { Project } from "../domain/project";
import { rpc } from "../lib/api-client";

const initialState: AudioEngineState = {
	status: "stopped",
	positionBeat: 0,
	loop: null,
	revision: null,
	audition: null,
	loudnessMatching: false,
};
const silentMeter: MasterMeter = {
	peak: 0,
	rms: 0,
	peakDb: -Infinity,
	rmsDb: -Infinity,
};

export const useBrowserAudio = (project: Project) => {
	const engineRef = useRef<DaiwAudioEngine | null>(null);
	const unsubscribeRef = useRef<(() => void) | null>(null);
	const unsubscribeFaultRef = useRef<(() => void) | null>(null);
	const snapshotRef = useRef<Snapshot | null>(null);
	const [state, setState] = useState<AudioEngineState>(initialState);
	const [meter, setMeter] = useState<MasterMeter>(silentMeter);
	const [busy, setBusy] = useState(false);
	const [snapshotRevision, setSnapshotRevision] = useState<string>();
	const [error, setError] = useState<string>();

	const ensureEngine = useCallback(() => {
		const current = engineRef.current;
		if (current !== null) return current;
		const engine = new DaiwAudioEngine();
		const snapshot = snapshotRef.current;
		if (snapshot !== null) engine.setSnapshot(snapshot);
		unsubscribeRef.current = engine.subscribe(setState);
		unsubscribeFaultRef.current = engine.subscribeFault((fault) =>
			setError(fault.message),
		);
		engineRef.current = engine;
		return engine;
	}, []);

	useEffect(() => {
		let active = true;
		const requestedRevision = project.revision;
		void rpc(
			{ projectId: project.id, method: "project.snapshot" },
			SnapshotResultSchema,
		)
			.then(({ result }) => {
				if (!active || result.snapshot.revision !== requestedRevision) return;
				snapshotRef.current = result.snapshot;
				setSnapshotRevision(result.snapshot.revision);
				engineRef.current?.setSnapshot(result.snapshot);
			})
			.catch((reason: unknown) => {
				if (active)
					setError(
						reason instanceof Error
							? reason.message
							: "Could not prepare the audio snapshot.",
					);
			});
		return () => {
			active = false;
		};
	}, [project.id, project.revision]);

	useEffect(() => {
		const engine = engineRef.current;
		if (state.status !== "playing") {
			if (engine !== null) setMeter(engine.getMeter());
			return;
		}
		let frame = 0;
		const sample = () => {
			const current = engineRef.current;
			if (current !== null) {
				const nextState = current.getState();
				setState(nextState);
				setMeter(current.getMeter());
				if (nextState.status === "playing")
					frame = window.requestAnimationFrame(sample);
			}
		};
		frame = window.requestAnimationFrame(sample);
		return () => window.cancelAnimationFrame(frame);
	}, [state.status]);

	useEffect(
		() => () => {
			unsubscribeRef.current?.();
			unsubscribeFaultRef.current?.();
			const engine = engineRef.current;
			engineRef.current = null;
			if (engine !== null) void engine.dispose();
		},
		[],
	);

	const prepare = useCallback(
		async (proposalId?: string) => {
			setBusy(true);
			setError(undefined);
			try {
				const engine = ensureEngine();
				if (proposalId === undefined) await engine.prepare(project.id);
				else await engine.prepareAB(project.id, proposalId);
				return true;
			} catch (reason) {
				setError(
					reason instanceof Error
						? reason.message
						: "Audio preparation failed.",
				);
				return false;
			} finally {
				setBusy(false);
			}
		},
		[ensureEngine, project.id],
	);

	const toggle = useCallback(async () => {
		const engine = ensureEngine();
		try {
			await engine.activate();
			if (state.audition === null && !(await prepare())) return;
			if (state.status === "playing") engine.pause();
			else await engine.play();
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : "Playback failed.");
		}
	}, [ensureEngine, prepare, state.audition, state.status]);

	const selectAB = useCallback(
		(side: AuditionSide) => {
			try {
				ensureEngine().selectAB(side);
			} catch (reason) {
				setError(
					reason instanceof Error ? reason.message : "A/B selection failed.",
				);
			}
		},
		[ensureEngine],
	);
	const toggleLoop = useCallback(() => {
		const snapshot = snapshotRef.current;
		if (snapshot === null || snapshot.length <= 0) {
			setError("Add material before enabling a loop.");
			return;
		}
		const engine = ensureEngine();
		if (state.loop !== null) {
			engine.setLoop(null);
			return;
		}
		engine.setLoop(
			fourBarRange(
				state.positionBeat,
				snapshot.length,
				project.meterMap ?? project.meter,
			),
		);
	}, [
		ensureEngine,
		project.meter,
		project.meterMap,
		state.loop,
		state.positionBeat,
	]);
	const seek = useCallback(
		(beat: number) => {
			try {
				ensureEngine().seek(beat);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : "Seeking failed.");
			}
		},
		[ensureEngine],
	);

	return {
		state,
		meter,
		busy: busy || snapshotRevision !== project.revision,
		error,
		toggle,
		prepareAB: (proposalId: string) => prepare(proposalId),
		selectAB,
		stop: () => engineRef.current?.stop(),
		seek,
		toggleLoop,
		setLoudnessMatching: (enabled: boolean) =>
			ensureEngine().setLoudnessMatching(enabled),
	};
};
