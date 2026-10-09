import { describe, expect, it } from "vitest";
import {
	beatAtTime,
	normalizeLoopBeat,
	secondsAtBeat,
	validateLoop,
} from "./transport";

describe("audio transport math", () => {
	it("round-trips sample-aligned beat offsets", () => {
		// Given
		const beat = 1.25;
		const tempo = 120;

		// When
		const seconds = secondsAtBeat(beat, tempo);

		// Then
		expect(beatAtTime(seconds, tempo)).toBe(beat);
	});

	it("wraps positions into an end-exclusive loop", () => {
		// Given
		const loop = { start: 4, end: 8 };

		// When
		const position = normalizeLoopBeat(10.5, loop);

		// Then
		expect(position).toBe(6.5);
	});

	it("rejects loop endpoints beyond the immutable snapshot", () => {
		// Given
		const loop = { start: 4, end: 9 };

		// When
		const validate = () => validateLoop(loop, 8);

		// Then
		expect(validate).toThrow(RangeError);
	});

	it("converts elapsed seconds through an absolute tempo-map anchor", () => {
		// Given
		const tempoMap = {
			points: [
				{ at: 0, bpm: 120, curve: "step" as const },
				{ at: 4, bpm: 60, curve: "step" as const },
			],
		};

		// When
		const seconds = secondsAtBeat(6, tempoMap);

		// Then
		expect(seconds).toBe(4);
		expect(beatAtTime(seconds, tempoMap)).toBe(6);
	});
});
