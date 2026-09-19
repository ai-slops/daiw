import { z } from "zod";
import {
	DefinitionSchema,
	DiagnosticSchema,
	InstanceSchema,
	ModifierSchema,
	NoteSchema,
	OperationSchema,
	ProjectSchema,
	TrackSchema,
} from "./project";

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

export const ExplainResultSchema = z.object({
	revision: z.string(),
	effectiveNotes: z.array(NoteSchema),
	instance: InstanceSchema,
	definition: DefinitionSchema,
	modifiers: z.array(ModifierSchema),
	overrides: z.array(OperationSchema),
});

export const DryRunResultSchema = z.object({
	executable: z.boolean(),
	affectedInstances: z.array(z.string()),
	hunks: z.array(z.record(z.string(), z.unknown())),
	diagnostics: z.array(DiagnosticSchema),
});

export const SnapshotSchema = z.object({
	revision: z.string(),
	tempo: z.number(),
	length: z.number(),
	tracks: z.array(TrackSchema),
	notes: z.array(
		NoteSchema.extend({
			instanceId: z.string(),
			trackId: z.string(),
			origin: z.object({
				definitionId: z.string(),
				noteId: z.string(),
				modifierIds: z.array(z.string()),
			}),
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
