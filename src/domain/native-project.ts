import { z } from "zod";

export const NativeProjectEnvelopeSchema = z.strictObject({
  format: z.literal("daiw-project"),
  formatVersion: z.literal(1),
  requiredFeatures: z.array(z.enum([
    "audio-regions-v1", "core-v1", "modifier-ordering-v1", "named-slots-v1",
    "note-patches-v1", "proposal-review-v1", "slot-bindings-v1",
  ])),
  contentHash: z.string().regex(/^blake3:[0-9a-f]{64}$/),
  content: z.record(z.string(), z.unknown()),
});

export type NativeProjectEnvelope = z.infer<typeof NativeProjectEnvelopeSchema>;
