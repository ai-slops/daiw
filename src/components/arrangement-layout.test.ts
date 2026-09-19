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
});
