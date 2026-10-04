import { describe, expect, it } from "vitest";
import {
	formatMusicalPosition,
	fourBarRange,
	nextBar,
	previousBar,
} from "./transport-timing";

describe("transport timing", () => {
	it("formats bars, denominator beats, and ticks in 3/8", () => {
		// Given
		const meter = [3, 8] as const;

		// When
		const position = formatMusicalPosition(1.25, meter);

		// Then
		expect(position).toBe("001 / 03 / 480");
	});

	it("navigates adjacent bars across a meter-map transition", () => {
		// Given
		const meterMap = {
			points: [
				{ at: 0, numerator: 4, denominator: 4 },
				{ at: 8, numerator: 5, denominator: 4 },
			],
		} as const;

		// When
		const backward = previousBar(8, meterMap);
		const forward = nextBar(8, meterMap);

		// Then
		expect(backward).toBe(4);
		expect(forward).toBe(13);
	});

	it("creates a four-bar loop using mapped bar boundaries", () => {
		// Given
		const meterMap = {
			points: [
				{ at: 0, numerator: 3, denominator: 4 },
				{ at: 6, numerator: 5, denominator: 4 },
			],
		} as const;

		// When
		const loop = fourBarRange(7, 30, meterMap);

		// Then
		expect(loop).toEqual({ start: 6, end: 26 });
	});
});
