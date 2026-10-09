import { z } from "zod";
import { validateMeterMap, validateTempoMap } from "../shared/timing.mjs";

const tempoSchema = z.number().min(20).max(400);
const meterSchema = z.tuple([
	z.number().int().min(1).max(255),
	z.number().int().min(1).max(255),
]);
const tempoMapSchema = z
	.object({
		points: z
			.array(
				z.object({
					at: z.number().nonnegative(),
					bpm: tempoSchema,
					curve: z.enum(["step", "linear"]),
				}),
			)
			.min(1)
			.max(1024),
	})
	.superRefine((map, context) => {
		try {
			validateTempoMap(map);
		} catch (error) {
			if (!(error instanceof RangeError)) throw error;
			context.addIssue({ code: "custom", message: error.message });
		}
	});
const meterMapSchema = z
	.object({
		points: z
			.array(
				z.object({
					at: z.number().nonnegative(),
					numerator: z.number().int().min(1).max(255),
					denominator: z.number().int().min(1).max(255),
				}),
			)
			.min(1)
			.max(1024),
	})
	.superRefine((map, context) => {
		try {
			validateMeterMap(map);
		} catch (error) {
			if (!(error instanceof RangeError)) throw error;
			context.addIssue({ code: "custom", message: error.message });
		}
	});
const snapshotSchema = z
	.looseObject({
		revision: z.string(),
		tempo: tempoSchema,
		meter: meterSchema.default([4, 4]),
		tempoMap: tempoMapSchema.optional(),
		meterMap: meterMapSchema.optional(),
		length: z.number().nonnegative(),
		regions: z.array(z.looseObject({ assetId: z.string() })).default([]),
	})
	.superRefine((snapshot, context) => {
		if (
			snapshot.tempoMap?.points[0]?.bpm !== undefined &&
			snapshot.tempoMap.points[0].bpm !== snapshot.tempo
		) {
			context.addIssue({
				code: "custom",
				path: ["tempoMap"],
				message: "tempo must equal the first tempo-map point",
			});
		}
		const firstMeter = snapshot.meterMap?.points[0];
		if (
			firstMeter !== undefined &&
			(firstMeter.numerator !== snapshot.meter[0] ||
				firstMeter.denominator !== snapshot.meter[1])
		) {
			context.addIssue({
				code: "custom",
				path: ["meterMap"],
				message: "meter must equal the first meter-map point",
			});
		}
	});

export function parseRenderSnapshot(value) {
	const result = z.object({ snapshot: snapshotSchema }).safeParse(value);
	return result.success
		? result.data.snapshot
		: snapshotSchema.parse(value);
}
