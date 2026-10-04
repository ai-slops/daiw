import { expect, it } from "vitest";
import { NativeProjectEnvelopeSchema } from "./native-project";
import { ProjectResultSchema } from "./api";

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

it("cannot be mistaken for a legacy project-result wrapper", () => {
  const project = { id: "p", name: "P", revision: "r", tempo: 120, meter: [4, 4], definitions: [], instances: [], modifiers: [], tracks: [], history: [], proposals: [] };
  expect(ProjectResultSchema.safeParse({ ...envelope, content: project }).success).toBe(false);
});
