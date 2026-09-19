import type { Project } from "../domain/project";

const minimumTimelineBeats = 32;
const beatsPerBar = 4;

export const timelineBeats = (project: Project): number => {
	const instanceEnd = project.instances.reduce(
		(end, instance) => Math.max(end, instance.start + instance.length),
		0,
	);
	const regionEnd = project.regions.reduce(
		(end, region) => Math.max(end, region.start + region.duration),
		0,
	);
	return Math.max(
		minimumTimelineBeats,
		Math.ceil(Math.max(instanceEnd, regionEnd) / beatsPerBar) * beatsPerBar,
	);
};

export const timelinePercent = (beat: number, totalBeats: number): string =>
	`${(beat / totalBeats) * 100}%`;
