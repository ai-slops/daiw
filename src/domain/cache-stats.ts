import { z } from "zod";

const CounterSchema = z.number().int().nonnegative();

const CacheCountersSchema = z.strictObject({
	hits: CounterSchema,
	misses: CounterSchema,
	evictions: CounterSchema,
	expandedWork: CounterSchema,
}).readonly();

export const CacheStatsSchema = z.strictObject({
	enabled: z.boolean(),
	limits: z.strictObject({
		maxEntries: CounterSchema,
		maxRetainedBytes: CounterSchema,
		maxNotes: CounterSchema,
		maxPathEntries: CounterSchema,
	}).readonly(),
	resident: z.strictObject({
		entries: CounterSchema,
		retainedBytes: CounterSchema,
		notes: CounterSchema,
		pathEntries: CounterSchema,
	}).readonly(),
	totals: CacheCountersSchema,
	lastRequest: CacheCountersSchema,
}).readonly();

export type CacheStats = z.infer<typeof CacheStatsSchema>;
