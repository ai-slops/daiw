import { describe, expect, it } from "vitest";
import { InstanceSchema } from "./project";
import { ExplainResultSchema } from "./api";

const instance = { id: "i", name: "I", definitionId: "d", trackId: "t", start: 0, length: 8, modifiers: [], overrides: [] };
const explanation = {
  revision: "r", instance, effectiveNotes: [], modifiers: [], overrides: [],
  definition: { id: "d", name: "D", length: 8, revision: 0, notes: [] },
};

describe("modifier ordering schema", () => {
  it("preserves explicit empty ordering without inventing it for legacy data", () => {
    const explicit = { ...instance, modifierOrdering: { edges: [] } };
    expect(InstanceSchema.parse(instance)).toEqual(instance);
    expect(InstanceSchema.parse(explicit)).toEqual(explicit);
  });

  it("rejects an ordering object without its required edges", () => {
    expect(InstanceSchema.safeParse({ ...instance, modifierOrdering: {} }).success).toBe(false);
  });

  it("preserves derived execution order and read/write resources", () => {
    const modifierPlan = {
      mode: "explicit", order: ["a", "b"], edges: [{ before: "a", after: "b" }],
      modifiers: [{ modifierId: "a", reads: [{ kind: "noteMembership" }], writes: [{ kind: "notePitch" }] }],
    };
    expect(ExplainResultSchema.parse({ ...explanation, modifierPlan })).toEqual({ ...explanation, modifierPlan });
  });

  it("rejects unrecognized derived access resources", () => {
    const modifierPlan = {
      mode: "legacy", order: ["a"],
      modifiers: [{ modifierId: "a", reads: [{ kind: "arbitraryMemory" }], writes: [] }],
    };
    expect(ExplainResultSchema.safeParse({ ...explanation, modifierPlan }).success).toBe(false);
  });
});
