import { beatAtSeconds, normalizeLoopBeat, secondsAtBeat } from "./transport";
import type { LoopRange, Snapshot } from "./types";

export const PLAYBACK_SAMPLE_RATE = 48_000;

export function frameAtBeat(snapshot: Snapshot, beat: number): number {
	return Math.round(
		secondsAtBeat(beat, snapshot.tempoMap ?? snapshot.tempo) *
			PLAYBACK_SAMPLE_RATE,
	);
}

export function beatAtFrame(snapshot: Snapshot, frame: number): number {
	return beatAtSeconds(
		frame / PLAYBACK_SAMPLE_RATE,
		snapshot.tempoMap ?? snapshot.tempo,
	);
}

export function snapshotFrames(snapshot: Snapshot): number {
	return frameAtBeat(snapshot, snapshot.length);
}

export function loopFrames(
	snapshot: Snapshot,
	loop: LoopRange | null,
): Readonly<{ start: number; end: number }> | null {
	if (loop === null) return null;
	return Object.freeze({
		start: frameAtBeat(snapshot, loop.start),
		end: frameAtBeat(snapshot, loop.end),
	});
}

export function normalizeFrame(
	frame: number,
	loop: Readonly<{ start: number; end: number }> | null,
): number {
	if (loop === null || frame < loop.end) return frame;
	return loop.start + ((frame - loop.start) % (loop.end - loop.start));
}

export function normalizedBeat(
	snapshot: Snapshot,
	beat: number,
	loop: LoopRange | null,
): number {
	return normalizeLoopBeat(Math.max(0, Math.min(beat, snapshot.length)), loop);
}
