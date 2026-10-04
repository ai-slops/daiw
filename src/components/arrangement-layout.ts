import type { MeterTiming } from "../../shared/timing.mjs";
import {
	barPositionAtBeat,
	nextBarBoundary,
	TICKS_PER_BEAT,
} from "../../shared/timing.mjs";
import type { Project } from "../domain/project";

const minimumTimelineBars = 8;
const maximumRulerMarkers = 1024;

export const projectMeterTiming = (project: Project): MeterTiming =>
	project.meterMap ?? project.meter;

const advanceBar = (beat: number, timing: MeterTiming): number =>
	nextBarBoundary(beat + 1 / TICKS_PER_BEAT, timing);

const beatAfterBars = (
	start: number,
	bars: number,
	timing: MeterTiming,
): number => {
	let beat = start;
	for (let index = 0; index < bars; index += 1) beat = advanceBar(beat, timing);
	return beat;
};

export const timelineBeats = (project: Project): number => {
	const instanceEnd = project.instances.reduce(
		(end, instance) => Math.max(end, instance.start + instance.length),
		0,
	);
	const regionEnd = project.regions.reduce(
		(end, region) => Math.max(end, region.start + region.duration),
		0,
	);
	const timing = projectMeterTiming(project);
	const contentEnd = Math.max(instanceEnd, regionEnd);
	const position = barPositionAtBeat(contentEnd, timing);
	const roundedContentEnd =
		Math.abs(contentEnd - position.barStart) < 1e-9
			? position.barStart
			: position.barEnd;
	return Math.max(
		beatAfterBars(0, minimumTimelineBars, timing),
		roundedContentEnd,
	);
};

export const timelineBars = (
	totalBeats: number,
	timing: MeterTiming,
): readonly number[] => {
	const bars = [0];
	let beat = 0;
	while (beat < totalBeats && bars.length < maximumRulerMarkers) {
		beat = advanceBar(beat, timing);
		bars.push(Math.min(beat, totalBeats));
	}
	return bars;
};

export const timelinePercent = (beat: number, totalBeats: number): string =>
	`${(beat / totalBeats) * 100}%`;
