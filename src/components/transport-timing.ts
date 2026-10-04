import type { Meter, MeterTiming } from "../../shared/timing.mjs";
import {
	barPositionAtBeat,
	nextBarBoundary,
	previousBarBoundary,
	TICKS_PER_BEAT,
} from "../../shared/timing.mjs";

export type BeatRange = Readonly<{ start: number; end: number }>;

export const meterAtBeat = (beat: number, timing: MeterTiming): Meter => {
	if (!("points" in timing)) return timing;
	const point = timing.points.findLast((candidate) => candidate.at <= beat);
	if (point === undefined)
		throw new RangeError("meter map must start at beat zero");
	return [point.numerator, point.denominator];
};

export const formatMusicalPosition = (
	beat: number,
	timing: MeterTiming,
): string => {
	const position = barPositionAtBeat(beat, timing);
	const [, denominator] = meterAtBeat(beat, timing);
	const denominatorBeat = position.beatInBar / (4 / denominator);
	const wholeBeat = Math.floor(denominatorBeat);
	const tick = Math.min(
		TICKS_PER_BEAT - 1,
		Math.round((denominatorBeat - wholeBeat) * TICKS_PER_BEAT),
	);
	return `${String(position.bar + 1).padStart(3, "0")} / ${String(wholeBeat + 1).padStart(2, "0")} / ${String(tick).padStart(3, "0")}`;
};

export const previousBar = (beat: number, timing: MeterTiming): number => {
	const boundary = previousBarBoundary(beat, timing);
	return Math.abs(boundary - beat) < 1e-9 && beat > 0
		? previousBarBoundary(Math.max(0, beat - 1 / TICKS_PER_BEAT), timing)
		: boundary;
};

export const nextBar = (beat: number, timing: MeterTiming): number =>
	nextBarBoundary(beat + 1 / TICKS_PER_BEAT, timing);

export const fourBarRange = (
	beat: number,
	length: number,
	timing: MeterTiming,
): BeatRange => {
	const boundary = previousBarBoundary(beat, timing);
	const start = boundary >= length ? previousBar(length, timing) : boundary;
	let end = start;
	for (let index = 0; index < 4; index += 1) end = nextBar(end, timing);
	return { start, end: Math.min(end, length) };
};
