import { z } from "zod";

const stateSchema = z
	.object({
		status: z.enum(["stopped", "paused", "playing", "faulted"]),
		positionFrame: z.number().int().nonnegative(),
		frames: z.number().int().nonnegative(),
		loop: z
			.object({ start: z.number().int(), end: z.number().int() })
			.nullable(),
		stats: z.object({
			underrunCallbacks: z.number().int().nonnegative(),
			staleBlocks: z.number().int().nonnegative(),
			deviceErrors: z.number().int().nonnegative(),
		}),
	})
	.strict();

export const nativeAudioOutputSchema = z.union([
	z
		.object({
			v: z.literal(1),
			type: z.literal("ready"),
			frames: z.number().int().nonnegative(),
		})
		.strict(),
	z
		.object({
			v: z.literal(1),
			type: z.literal("response"),
			id: z.number().int().nonnegative(),
			ok: z.literal(true),
			state: stateSchema,
		})
		.strict(),
	z
		.object({
			v: z.literal(1),
			type: z.literal("response"),
			id: z.number().int().nonnegative(),
			ok: z.literal(false),
			error: z.object({ code: z.string(), message: z.string() }).strict(),
		})
		.strict(),
	z
		.object({
			v: z.literal(1),
			type: z.literal("state"),
			sequence: z.number().int().nonnegative(),
			state: stateSchema,
		})
		.strict(),
	z
		.object({
			v: z.literal(1),
			type: z.literal("fatal"),
			error: z.object({ code: z.string(), message: z.string() }).strict(),
		})
		.strict(),
]);
