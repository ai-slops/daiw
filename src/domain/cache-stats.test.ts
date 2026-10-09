import { describe, expect, it } from "vitest";
import { CacheStatsSchema } from "./cache-stats";

const stats = {
	enabled: true,
	limits: { maxEntries: 64, maxRetainedBytes: 1_048_576, maxNotes: 4_096, maxPathEntries: 8_192 },
	resident: { entries: 2, retainedBytes: 512, notes: 8, pathEntries: 12 },
	totals: { hits: 3, misses: 2, evictions: 0, expandedWork: 18 },
	lastRequest: { hits: 2, misses: 0, evictions: 0, expandedWork: 0 },
};

describe("CacheStatsSchema", () => {
	it("parses the exact read-only worker diagnostics contract", () => {
		// Given / When
		const parsed = CacheStatsSchema.parse(stats);

		// Then
		expect(parsed).toEqual(stats);
	});

	it("rejects negative, fractional, and non-finite counters", () => {
		// Given
		const invalidCounters = [-1, 0.5, Number.POSITIVE_INFINITY];

		// When
		const results = invalidCounters.map((hits) => CacheStatsSchema.safeParse({
			...stats,
			totals: { ...stats.totals, hits },
		}).success);

		// Then
		expect(results).toEqual([false, false, false]);
	});

	it("rejects credentials and canonical cache payloads at every object boundary", () => {
		// Given
		const invalid = [
			{ ...stats, token: "secret" },
			{ ...stats, resident: { ...stats.resident, entries: 2, cache: [{ id: "canonical-project" }] } },
			{ ...stats, lastRequest: { ...stats.lastRequest, notes: [{ id: "note-1" }] } },
		];

		// When
		const results = invalid.map((value) => CacheStatsSchema.safeParse(value).success);

		// Then
		expect(results).toEqual([false, false, false]);
	});
});
