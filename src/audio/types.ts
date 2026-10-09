import { z } from "zod";
import { NoteOriginSchema } from "../domain/api";
import { MeterMapSchema, MeterSchema, TempoMapSchema, TempoSchema } from "../domain/project";

const instrumentSchema = z.enum(["synth", "bass", "kick", "snare", "hat", "pad", "sample"]);

const trackSchema = z.object({
  id: z.string(),
  name: z.string(),
  role: z.string(),
  color: z.string(),
  instrument: instrumentSchema,
  gain: z.number(),
  pan: z.number(),
  mute: z.boolean(),
  solo: z.boolean(),
  cutoff: z.number(),
});

const effectiveNoteSchema = z.object({
  id: z.string(),
  start: z.number(),
  duration: z.number(),
  pitch: z.number(),
  velocity: z.number(),
  instanceId: z.string(),
  trackId: z.string(),
  origin: NoteOriginSchema,
});

const audioRegionSchema = z.object({
  id: z.string(),
  trackId: z.string(),
  assetId: z.string(),
  start: z.number(),
  duration: z.number(),
  gain: z.number(),
  sourceOffset: z.number(),
});

export const snapshotSchema = z.object({
  revision: z.string(),
  tempo: TempoSchema,
  meter: MeterSchema.default([4, 4]),
  tempoMap: TempoMapSchema.optional(),
  meterMap: MeterMapSchema.optional(),
  length: z.number().nonnegative(),
  tracks: z.array(trackSchema),
  notes: z.array(effectiveNoteSchema),
  regions: z.array(audioRegionSchema).default([]),
}).superRefine((snapshot, context) => {
  if (snapshot.tempoMap !== undefined && snapshot.tempoMap.points[0]?.bpm !== snapshot.tempo) {
    context.addIssue({ code: "custom", path: ["tempoMap"], message: "tempo must equal the first tempo-map point" });
  }
  const firstMeter = snapshot.meterMap?.points[0];
  if (firstMeter !== undefined && (firstMeter.numerator !== snapshot.meter[0] || firstMeter.denominator !== snapshot.meter[1])) {
    context.addIssue({ code: "custom", path: ["meterMap"], message: "meter must equal the first meter-map point" });
  }
});

export type Snapshot = Readonly<z.infer<typeof snapshotSchema>>;
export type Track = Readonly<z.infer<typeof trackSchema>>;
export type EffectiveNote = Readonly<z.infer<typeof effectiveNoteSchema>>;
export type LoopRange = Readonly<{ start: number; end: number }>;
export type AuditionSide = "a" | "b";
export type TransportStatus = "stopped" | "playing" | "paused";

export type AudioEngineState = Readonly<{
  status: TransportStatus;
  positionBeat: number;
  loop: LoopRange | null;
  revision: string | null;
  audition: AuditionSide | null;
  loudnessMatching: boolean;
}>;

export type StateListener = (state: AudioEngineState) => void;

export type MasterMeter = Readonly<{
  peak: number;
  rms: number;
  peakDb: number;
  rmsDb: number;
}>;
