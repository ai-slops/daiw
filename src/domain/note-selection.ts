import { z } from "zod";

const EntityIdSchema = z.string().min(1).max(256).brand<"EntityId">();
const RevisionSchema = z.string().min(1).brand<"Revision">();

export const NoteSelectorSchema = z.discriminatedUnion("type", [
	z.object({ type: z.literal("all") }),
	z.object({
		type: z.literal("stableIds"),
		noteIds: z
			.array(EntityIdSchema)
			.min(1)
			.max(4096)
			.refine((noteIds) => new Set(noteIds).size === noteIds.length, {
				message: "stable note IDs must be unique",
			}),
	}),
	z.object({ type: z.literal("namedSlot"), slot: z.string().min(1).max(256) }),
	z
		.object({
			type: z.literal("beatRange"),
			start: z.number().nonnegative(),
			end: z.number().positive(),
		})
		.refine(({ start, end }) => start < end, {
			message: "beat range must increase",
		}),
	z
		.object({
			type: z.literal("pitchRange"),
            min: z.number(),
            max: z.number(),
		})
		.refine(({ min, max }) => min <= max, {
			message: "pitch range must be ordered",
		}),
]);

export const QueryNotesParamsSchema = z.object({
	instanceId: EntityIdSchema,
	scope: z.enum(["source", "instance"]),
	selector: NoteSelectorSchema,
	maxResults: z.number().int().min(1).max(4096),
});

export const SelectionHandleSchema = z
	.object({
		projectId: EntityIdSchema,
		baseRevision: RevisionSchema,
		definitionId: EntityIdSchema,
		definitionRevision: z.number().int().nonnegative(),
		instanceId: EntityIdSchema,
		scope: z.enum(["source", "instance"]),
		selector: NoteSelectorSchema,
		noteIds: z.array(EntityIdSchema).max(4096),
		cardinality: z.number().int().nonnegative().max(4096),
		resolutionFingerprint: z.string().min(1),
	})
	.refine(({ cardinality, noteIds }) => cardinality === noteIds.length, {
		message: "cardinality must match resolved note IDs",
	});

export const NoteTransformSchema = z.discriminatedUnion("type", [
	z.object({ type: z.literal("transpose"), semitones: z.number() }),
	z.object({
		type: z.literal("setVelocity"),
		velocity: z.number().min(0).max(1),
	}),
]);

export const TransformSelectedNotesCommandSchema = z.object({
	type: z.literal("transformSelectedNotes"),
	selection: SelectionHandleSchema,
	transform: NoteTransformSchema,
});

export const QueryNotesResultSchema = z.object({
	selection: SelectionHandleSchema,
});

export type NoteSelector = z.infer<typeof NoteSelectorSchema>;
export type QueryNotesParams = z.infer<typeof QueryNotesParamsSchema>;
export type SelectionHandle = z.infer<typeof SelectionHandleSchema>;
export type NoteTransform = z.infer<typeof NoteTransformSchema>;
