import { expect, it } from "vitest";
import { NativeProjectEnvelopeSchema } from "./native-project";
import { ProjectResultSchema, SnapshotSchema } from "./api";

const envelope = {
  format: "daiw-project", formatVersion: 1, requiredFeatures: ["core-v1"],
  contentHash: `blake3:${"0".repeat(64)}`,
  content: { id: "p", futureSemanticField: { nested: [1, 2] } },
};

it("preserves content opaquely until the core validates compatibility", () => {
  expect(NativeProjectEnvelopeSchema.parse(envelope)).toEqual(envelope);
});

it("rejects unsupported versions and required capabilities", () => {
  expect(NativeProjectEnvelopeSchema.safeParse({ ...envelope, formatVersion: 9 }).success).toBe(false);
  expect(NativeProjectEnvelopeSchema.safeParse({ ...envelope, requiredFeatures: ["future"] }).success).toBe(false);
});

it("accepts tempo and meter map native feature requirements", () => {
  expect(NativeProjectEnvelopeSchema.safeParse({
    ...envelope,
    requiredFeatures: ["core-v1", "tempo-map-v1", "meter-map-v1"],
  }).success).toBe(true);
});

it("accepts the nested definitions native feature requirement", () => {
  // Given / When
  const result = NativeProjectEnvelopeSchema.safeParse({
    ...envelope,
    requiredFeatures: ["core-v1", "nested-definitions-v1"],
  });

  // Then
  expect(result.success).toBe(true);
});

it("preserves optional nested note provenance paths in snapshots", () => {
  // Given
  const snapshot = {
    revision: "r", tempo: 120, length: 8, tracks: [], regions: [],
    notes: [{
      id: "rendered-note", start: 1, duration: 1, pitch: 64, velocity: 0.8,
      instanceId: "instance-1", trackId: "track-1",
      origin: {
        definitionId: "leaf", noteId: "leaf-note", modifierIds: ["modifier-1"],
        definitionPath: [
          { definitionId: "root", definitionRevision: 2 },
          { definitionId: "leaf", definitionRevision: 4, placementId: "placement-1", loopIndex: 1 },
        ],
        instanceLoopIndex: 2,
      },
    }],
  };

  // When
  const parsed = SnapshotSchema.parse(snapshot);

  // Then
  expect(parsed.notes[0]?.origin).toEqual(snapshot.notes[0]?.origin);
});

it("cannot be mistaken for a legacy project-result wrapper", () => {
  const project = { id: "p", name: "P", revision: "r", tempo: 120, meter: [4, 4], definitions: [], instances: [], modifiers: [], tracks: [], history: [], proposals: [] };
  expect(ProjectResultSchema.safeParse({ ...envelope, content: project }).success).toBe(false);
});

it("parses mapped snapshot timing while retaining legacy scalar snapshots", () => {
  const base = { revision: "r", tempo: 120, length: 8, tracks: [], notes: [], regions: [] };
  expect(SnapshotSchema.safeParse(base).success).toBe(true);
  expect(SnapshotSchema.safeParse({
    ...base,
    tempoMap: { points: [{ at: 0, bpm: 120, curve: "linear" }, { at: 4, bpm: 90, curve: "step" }] },
    meterMap: { points: [{ at: 0, numerator: 4, denominator: 4 }, { at: 4, numerator: 3, denominator: 4 }] },
  }).success).toBe(true);
});
