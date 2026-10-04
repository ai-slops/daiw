import { describe, expect, it } from "vitest";
import { ProjectSchema } from "../domain/project";
import { timelineBeats } from "./arrangement-layout";

describe("timelineBeats", () => {
	it("includes audio-region extents when they exceed MIDI instances", () => {
		// Given
		const project = ProjectSchema.parse({
			id: "project-1",
			name: "Audio arrangement",
			revision: "rev-1",
			tempo: 120,
			meter: [4, 4],
			definitions: [],
			instances: [],
			tracks: [],
			modifiers: [],
			proposals: [],
			history: [],
			assets: [],
			regions: [
				{
					id: "region-1",
					trackId: "track-1",
					assetId: "asset-1",
					start: 30,
					duration: 10,
					gain: 0,
					sourceOffset: 0,
				},
			],
		});

		// When
		const beats = timelineBeats(project);

		// Then
		expect(beats).toBe(40);
	});

	it("rounds the timeline to the next bar when the meter is not 4/4", () => {
		// Given
		const project = ProjectSchema.parse({
			id: "project-3-4",
			name: "Waltz arrangement",
			revision: "rev-1",
			tempo: 120,
			meter: [3, 4],
			definitions: [],
			instances: [],
			tracks: [],
			modifiers: [],
			proposals: [],
			history: [],
			assets: [],
			regions: [
				{
					id: "region-1",
					trackId: "track-1",
					assetId: "asset-1",
					start: 30,
					duration: 1.1,
					gain: 0,
					sourceOffset: 0,
				},
			],
		});

		// When
		const beats = timelineBeats(project);

		// Then
		expect(beats).toBe(33);
	});

	it("rounds across a meter-map transition using the active bar length", () => {
		// Given
		const project = ProjectSchema.parse({
			id: "project-meter-map",
			name: "Mixed meter arrangement",
			revision: "rev-1",
			tempo: 120,
			meter: [4, 4],
			meterMap: {
				points: [
					{ at: 0, numerator: 4, denominator: 4 },
					{ at: 8, numerator: 5, denominator: 4 },
				],
			},
			definitions: [],
			instances: [],
			tracks: [],
			modifiers: [],
			proposals: [],
			history: [],
			assets: [],
			regions: [
				{
					id: "region-1",
					trackId: "track-1",
					assetId: "asset-1",
					start: 11,
					duration: 3,
					gain: 0,
					sourceOffset: 0,
				},
			],
		});

		// When
		const beats = timelineBeats(project);

		// Then
		expect(beats).toBe(38);
	});
});
