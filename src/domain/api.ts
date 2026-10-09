import { z } from "zod";
import {
	DefinitionSchema,
	DiagnosticSchema,
	InstanceSchema,
	ModifierSchema,
	ModifierOrderingSchema,
	MeterMapSchema,
	MeterSchema,
	NoteSchema,
	OperationSchema,
	ProjectSchema,
	TempoMapSchema,
	TempoSchema,
	TrackSchema,
} from "./project";

export const DefinitionPathEntrySchema = z.object({
	definitionId: z.string().min(1),
	definitionRevision: z.number().int().nonnegative(),
	placementId: z.string().min(1).optional(),
	loopIndex: z.number().int().nonnegative().optional(),
});

export const NoteOriginSchema = z.object({
	definitionId: z.string().min(1),
	noteId: z.string().min(1),
	modifierIds: z.array(z.string().min(1)),
	definitionPath: z.array(DefinitionPathEntrySchema).optional(),
	instanceLoopIndex: z.number().int().nonnegative().optional(),
});

export const CompositionNoteSchema = NoteSchema.extend({
	origin: NoteOriginSchema,
});

export const SessionSchema = z.object({ token: z.string().min(1) });
export const HealthSchema = z.object({
	status: z.literal("ok"),
	protocolVersion: z.number().int().positive(),
});
export const ProjectSummarySchema = z.object({
	id: z.string().min(1),
	name: z.string(),
	revision: z.string(),
	updatedAt: z.string(),
});
export const ProjectListSchema = z.object({
	projects: z.array(ProjectSummarySchema),
});

const AccessResourceSchema = z.discriminatedUnion("kind", [
	z.object({ kind: z.enum(["noteMembership", "noteTiming", "notePitch", "noteVelocity", "trackGain"]) }),
	z.object({ kind: z.literal("parameter"), parameter: z.enum(["cutoff", "pan"]) }),
	z.object({ kind: z.literal("slot"), slot: z.string().min(1) }),
]);

const ModifierPlanSchema = z.object({
	mode: z.enum(["legacy", "explicit"]),
	order: z.array(z.string().min(1)),
	edges: ModifierOrderingSchema.shape.edges.optional(),
	modifiers: z.array(z.object({
		modifierId: z.string().min(1),
		reads: z.array(AccessResourceSchema),
		writes: z.array(AccessResourceSchema),
	})),
});

export const ExplainResultSchema = z.object({
	revision: z.string(),
	effectiveNotes: z.array(NoteSchema),
	instance: InstanceSchema,
	definition: DefinitionSchema,
	modifiers: z.array(ModifierSchema),
	overrides: z.array(OperationSchema),
	unboundModifierIds: z.array(z.string().min(1)).optional(),
	modifierPlan: ModifierPlanSchema.optional(),
	compositionNotes: z.array(CompositionNoteSchema).optional(),
});

const InstanceCheckSchema = z.discriminatedUnion("status", [
	z.object({ status: z.literal("absent") }),
	z.object({
		status: z.literal("clean"),
		definitionId: z.string().min(1),
		definitionRevision: z.number().int().nonnegative(),
		noteCount: z.number().int().nonnegative(),
		unboundModifierIds: z.array(z.string().min(1)).optional(),
	}),
	z.object({
		status: z.literal("conflict"),
		definitionId: z.string().min(1),
		definitionRevision: z.number().int().nonnegative().nullable(),
		diagnostic: DiagnosticSchema,
		unboundModifierIds: z.array(z.string().min(1)).optional(),
		binding: z.object({
			modifierId: z.string().min(1),
			reason: z.enum([
				"definitionChanged", "sourceRevisionChanged", "selectorsChanged",
				"slotMissing", "rangeChanged", "cardinalityChanged", "targetsChanged",
			]),
			slot: z.string().min(1).optional(),
		}).optional(),
		target: z.object({
			kind: z.enum(["missing", "ambiguous", "addedIdCollision"]),
			noteId: z.string().min(1),
		}).optional(),
		actions: z.array(z.enum([
			"reviseCommand", "restoreMissingNote", "editInstanceNotes",
			"chooseUniqueNoteIds", "reviewModifiers", "rebaseModifier",
		])),
	}),
]);

const BeatRangeSchema = z.object({
	start: z.number().nonnegative(),
	end: z.number().nonnegative(),
});

export const DryRunResultSchema = z.object({
	executable: z.boolean(),
	affectedInstances: z.array(z.string()),
	hunks: z.array(z.record(z.string(), z.unknown())),
	diagnostics: z.array(DiagnosticSchema),
	impact: z.object({
		baseRevision: z.string().min(1),
		affectedDefinitions: z.array(z.string().min(1)),
		affectedTracks: z.array(z.string().min(1)),
		previewRanges: z.array(z.object({
			instanceId: z.string().min(1),
			before: BeatRangeSchema.nullable(),
			after: BeatRangeSchema.nullable(),
		})),
	}).optional(),
	noteResolution: z.object({
		ready: z.boolean(),
		instances: z.array(z.object({
			instanceId: z.string().min(1),
			before: InstanceCheckSchema,
			after: InstanceCheckSchema,
		})),
	}).optional(),
});

export const SnapshotSchema = z.object({
	revision: z.string(),
	tempo: TempoSchema,
	meter: MeterSchema.default([4, 4]),
	tempoMap: TempoMapSchema.optional(),
	meterMap: MeterMapSchema.optional(),
	length: z.number(),
	tracks: z.array(TrackSchema),
	notes: z.array(
		NoteSchema.extend({
			instanceId: z.string(),
			trackId: z.string(),
			origin: NoteOriginSchema,
		}),
	),
	regions: z.array(
		z.object({
			id: z.string(),
			trackId: z.string(),
			assetId: z.string(),
			start: z.number(),
			duration: z.number(),
			gain: z.number(),
			sourceOffset: z.number(),
		}),
	),
}).superRefine((snapshot, context) => {
	if (snapshot.tempoMap !== undefined && snapshot.tempoMap.points[0]?.bpm !== snapshot.tempo) {
		context.addIssue({ code: "custom", path: ["tempoMap"], message: "tempo must equal the first tempo-map point" });
	}
	const firstMeter = snapshot.meterMap?.points[0];
	if (firstMeter !== undefined && (firstMeter.numerator !== snapshot.meter[0] || firstMeter.denominator !== snapshot.meter[1])) {
		context.addIssue({ code: "custom", path: ["meterMap"], message: "meter must equal the first meter-map point" });
	}
});
export const SnapshotResultSchema = z.object({ snapshot: SnapshotSchema });
export const WaveformSchema = z.object({
	peaks: z.array(z.number().min(0).max(1)),
	frames: z.number().int().positive(),
	sampleRate: z.number().int().positive(),
	channels: z.number().int().positive(),
});

export const ProjectResultSchema = z
	.object({ project: ProjectSchema })
	.or(ProjectSchema);
export type ProjectSummary = z.infer<typeof ProjectSummarySchema>;
export type DryRunResult = z.infer<typeof DryRunResultSchema>;
export type ExplainResult = z.infer<typeof ExplainResultSchema>;
export type CompositionNote = z.infer<typeof CompositionNoteSchema>;
