import { describe, expect, it } from "vitest";
import { ProjectSchema, RpcResponseSchema } from "./project";

const blankProject = {
  id: "project-1",
  name: "Night Transit",
  revision: "rev-1",
  tempo: 124,
  meter: [4, 4],
  definitions: [],
  instances: [],
  tracks: [],
  modifiers: [],
  proposals: [],
  history: [],
};

describe("ProjectSchema", () => {
  it("parses a canonical blank project when the API returns the contract shape", () => {
    // Given a project-shaped API payload
    // When the payload crosses the Zod boundary
    const result = ProjectSchema.safeParse(blankProject);

    // Then the canonical project is accepted
    expect(result.success).toBe(true);
  });

  it("rejects an unknown operation when the API returns an unsupported variant", () => {
    // Given a payload with a non-contract operation
    const invalid = {
      ...blankProject,
      modifiers: [{ id: "mod-1", name: "Mystery", operations: [{ type: "warp", amount: 2 }] }],
    };

    // When the payload crosses the Zod boundary
    const result = ProjectSchema.safeParse(invalid);

    // Then the unsupported operation is rejected
    expect(result.success).toBe(false);
  });

  it("parses normalized velocity and numeric definition revisions from canonical Rust state", () => {
    // Given a project containing a canonical source definition
    const project = {
      ...blankProject,
      definitions: [{
        id: "definition-1",
        name: "Pulse",
        length: 4,
        revision: 2,
        notes: [{ id: "note-1", start: 0, duration: 1, pitch: 60, velocity: 0.8 }],
      }],
    };

    // When the project crosses the client boundary
    const parsed = ProjectSchema.parse(project);

    // Then semantic numeric units are preserved
    expect(parsed.definitions[0]?.notes[0]?.velocity).toBe(0.8);
  });
});

describe("RpcResponseSchema", () => {
  it("keeps a typed worker error visible when the command fails", () => {
    // Given a failed worker response
    const response = {
      id: "rpc-1",
      ok: false,
      error: { code: "RevisionConflict", message: "The project changed. Reload and try again." },
    };

    // When the envelope is parsed
    const parsed = RpcResponseSchema.parse(response);

    // Then the machine error code remains available to the UI
    expect(parsed.ok ? "unexpected-success" : parsed.error.code).toBe("RevisionConflict");
  });
});
