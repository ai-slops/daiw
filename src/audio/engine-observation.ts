import { AudioEngineError } from "./errors";
import { loopFrames, normalizeFrame, snapshotFrames } from "./playback-frames";
import { CHUNK_FRAMES } from "./server-render";
import { STREAM_BUFFER_CAP, type StreamBranch } from "./stream-branch";
import { STREAM_BUFFER_BUDGET, type StreamBufferBudget } from "./stream-budget";
import type { AuditionSide, LoopRange, Snapshot } from "./types";

export function auditionGain(
	streams: ReadonlyMap<AuditionSide, StreamBranch>,
	audition: AuditionSide | null,
	enabled: boolean,
): number {
	const selected = audition === null ? undefined : streams.get(audition);
	const reference = streams.get("a");
	if (
		!enabled ||
		audition !== "b" ||
		selected === undefined ||
		reference === undefined
	)
		return 1;
	const candidateRms = selected.descriptor.info.pcmRms;
	if (candidateRms === 0 || reference.descriptor.info.pcmRms === 0) return 1;
	return Math.max(
		0.25,
		Math.min(4, reference.descriptor.info.pcmRms / candidateRms),
	);
}

export function streamingMetrics(
	streams: ReadonlyMap<AuditionSide, StreamBranch>,
	budget: StreamBufferBudget,
) {
	const empty = {
		uniqueBuffers: 0,
		cachedChunks: 0,
		scheduledBuffers: 0,
		inflightRanges: 0,
		retainedBytes: 0,
	};
	return Object.freeze({
		maxUniqueBuffersPerBranch: STREAM_BUFFER_CAP,
		maxUniqueBuffersOverall: STREAM_BUFFER_BUDGET,
		overall: budget.metrics(),
		chunkFrames: CHUNK_FRAMES,
		branches: {
			a: streams.get("a")?.metrics() ?? empty,
			b: streams.get("b")?.metrics() ?? empty,
		},
	});
}

export function requiredStream(
	streams: ReadonlyMap<AuditionSide, StreamBranch>,
	audition: AuditionSide | null,
): StreamBranch {
	if (audition === null)
		throw new AudioEngineError("prepare must complete before playback");
	const stream = streams.get(audition);
	if (stream === undefined)
		throw new AudioEngineError("selected server render is unavailable");
	return stream;
}

type PlaybackClock = Readonly<{
	snapshot: Snapshot;
	anchorFrame: number;
	anchorWhen: number;
	currentTime: number;
	loop: LoopRange | null;
}>;

export function playbackFrame(clock: PlaybackClock): number {
	const elapsed = Math.max(
		0,
		Math.round((clock.currentTime - clock.anchorWhen) * 48_000),
	);
	return Math.min(
		snapshotFrames(clock.snapshot),
		normalizeFrame(
			clock.anchorFrame + elapsed,
			loopFrames(clock.snapshot, clock.loop),
		),
	);
}
