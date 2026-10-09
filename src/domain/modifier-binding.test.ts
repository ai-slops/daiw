import { describe, expect, it } from "vitest";
import { InstanceSchema } from "./project";
import { DryRunResultSchema } from "./api";

const instance = {
  id: "i1", name: "First", definitionId: "d1", trackId: "t1",
  start: 0, length: 8, modifiers: ["m1"], overrides: [],
};
const binding = {
  definitionId: "d1", definitionRevision: 2,
  slots: { ending: { range: { start: 6, end: 8 }, noteIds: ["n1"], cardinality: 1 } },
};

describe("modifier binding wire contract", () => {
  it("preserves persisted source targets and range on an instance", () => {
    const payload = {
      ...instance,
      modifierBindings: {
        m1: {
          ...binding,
          definitionRevisions: [{ definitionId: "d1", revision: 2 }, { definitionId: "leaf", revision: 5 }],
        },
      },
    };
    expect(InstanceSchema.parse(payload)).toEqual(payload);
  });

  it("does not invent a verified binding for an older instance", () => {
    expect(InstanceSchema.parse(instance)).toEqual(instance);
  });

  it("rejects incomplete nested binding metadata", () => {
    const malformed = { ...instance, modifierBindings: { m1: { definitionId: "d1" } } };
    expect(InstanceSchema.safeParse(malformed).success).toBe(false);
  });

  it("preserves stale binding diagnostics and legacy-unbound reports", () => {
    const before = {
      status: "clean", definitionId: "d1", definitionRevision: 2,
      noteCount: 4, unboundModifierIds: ["legacy"],
    };
    const after = {
      status: "conflict", definitionId: "d1", definitionRevision: 3,
      diagnostic: { code: "RebaseConflict", severity: "blocking", message: "Binding changed" },
      binding: { modifierId: "m1", reason: "sourceRevisionChanged" },
      actions: ["rebaseModifier"], unboundModifierIds: ["legacy"],
    };
    const payload = {
      executable: true, affectedInstances: ["i1"], hunks: [], diagnostics: [],
      noteResolution: { ready: false, instances: [{ instanceId: "i1", before, after }] },
    };
    expect(DryRunResultSchema.parse(payload)).toEqual(payload);
  });
});
