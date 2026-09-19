import { z } from "zod";

const IdSchema = z.string().min(1).brand<"EntityId">();
const RevisionSchema = z.string().min(1).brand<"Revision">();

export const NoteSchema = z.object({
  id: IdSchema,
  start: z.number().nonnegative(),
  duration: z.number().positive(),
  pitch: z.number().min(0).max(127),
  velocity: z.number().min(0).max(1),
});

const TransposeSchema = z.object({ type: z.literal("transpose"), semitones: z.number() });
const VelocitySchema = z.object({ type: z.literal("velocity"), factor: z.number().positive() });
const GainSchema = z.object({ type: z.literal("gain"), db: z.number() });
const AddNotesSchema = z.object({ type: z.literal("addNotes"), notes: z.array(NoteSchema) });
const ReplaceSlotSchema = z.object({
  type: z.literal("replaceSlot"),
  slot: z.string().min(1),
  notes: z.array(NoteSchema),
});
const SetParameterSchema = z.object({
  type: z.literal("setParameter"),
  parameter: z.enum(["cutoff", "pan"]),
  value: z.number(),
});

export const OperationSchema = z.discriminatedUnion("type", [
  TransposeSchema,
  VelocitySchema,
  GainSchema,
  AddNotesSchema,
  ReplaceSlotSchema,
  SetParameterSchema,
]);

export const TrackSchema = z.object({
  id: IdSchema,
  name: z.string().min(1),
  role: z.string(),
  color: z.string(),
  instrument: z.enum(["synth", "bass", "kick", "snare", "hat", "pad", "sample"]),
  gain: z.number(),
  pan: z.number().min(-1).max(1),
  mute: z.boolean(),
  solo: z.boolean(),
  cutoff: z.number().positive(),
});

export const DefinitionSchema = z.object({
  id: IdSchema,
  name: z.string().min(1),
  length: z.number().positive(),
  notes: z.array(NoteSchema),
  revision: z.number().int().nonnegative(),
});

export const InstanceSchema = z.object({
  id: IdSchema,
  name: z.string(),
  definitionId: IdSchema,
  trackId: IdSchema,
  start: z.number().nonnegative(),
  length: z.number().positive(),
  modifiers: z.array(IdSchema),
  overrides: z.array(OperationSchema),
});

export const ModifierSchema = z.object({
  id: IdSchema,
  name: z.string().min(1),
  operations: z.array(OperationSchema),
});

export const HunkSchema = z.object({
  id: IdSchema,
  title: z.string(),
  description: z.string(),
  operation: OperationSchema,
  dependencies: z.array(IdSchema),
  accepted: z.boolean(),
});

export const ProposalSchema = z.object({
  id: IdSchema,
  name: z.string(),
  request: z.string(),
  baseRevision: RevisionSchema,
  instanceId: IdSchema,
  status: z.enum(["pending", "accepted", "partiallyAccepted", "rejected"]),
  hunks: z.array(HunkSchema),
});

export const AssetSchema = z.object({
  id: IdSchema,
  name: z.string(),
  hash: z.string(),
  frames: z.number().int().nonnegative(),
  sampleRate: z.number().int().positive(),
  channels: z.number().int().positive(),
});

export const AudioRegionSchema = z.object({
  id: IdSchema,
  trackId: IdSchema,
  assetId: IdSchema,
  start: z.number().nonnegative(),
  duration: z.number().positive(),
  gain: z.number(),
  sourceOffset: z.number().nonnegative(),
});

export const ProjectSchema = z.object({
  id: IdSchema,
  name: z.string().min(1),
  revision: RevisionSchema,
  tempo: z.number().positive(),
  meter: z.tuple([z.literal(4), z.literal(4)]),
  definitions: z.array(DefinitionSchema),
  instances: z.array(InstanceSchema),
  tracks: z.array(TrackSchema),
  modifiers: z.array(ModifierSchema),
  proposals: z.array(ProposalSchema),
  history: z.array(z.object({ id: IdSchema, message: z.string(), author: z.string(), revision: RevisionSchema })),
  regions: z.array(AudioRegionSchema).default([]),
  assets: z.array(AssetSchema).default([]),
});

export const DiagnosticSchema = z.object({
  code: z.string(),
  severity: z.enum(["blocking", "warning", "info"]),
  message: z.string(),
  entityId: IdSchema.optional(),
});

const WorkerErrorSchema = z.object({ code: z.string(), message: z.string() });
export const RpcResponseSchema = z.discriminatedUnion("ok", [
  z.object({ id: z.string(), ok: z.literal(true), state: ProjectSchema, result: z.record(z.string(), z.unknown()) }),
  z.object({ id: z.string().optional(), ok: z.literal(false), error: WorkerErrorSchema }),
]);

export const rpcResponseSchema = <Output extends z.ZodType>(result: Output) =>
  z.discriminatedUnion("ok", [
    z.object({ id: z.string(), ok: z.literal(true), state: ProjectSchema, result }),
    z.object({ id: z.string().optional(), ok: z.literal(false), error: WorkerErrorSchema }),
  ]);

export type Project = z.infer<typeof ProjectSchema>;
export type Track = z.infer<typeof TrackSchema>;
export type Instance = z.infer<typeof InstanceSchema>;
export type Definition = z.infer<typeof DefinitionSchema>;
export type Modifier = z.infer<typeof ModifierSchema>;
export type Proposal = z.infer<typeof ProposalSchema>;
export type Note = z.infer<typeof NoteSchema>;
export type Operation = z.infer<typeof OperationSchema>;
export type EntityId = z.infer<typeof IdSchema>;
