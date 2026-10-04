import { describe, expect, it } from "vitest";
import { HunkSchema, ModifierSchema, OperationSchema, ProjectSchema, RpcResponseSchema } from "./project";

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
  it("accepts every finite pitch persisted by the core wire contract", () => {
    const project = {
      ...blankProject,
      definitions: [{
        id: "definition-1",
        name: "Wide pitch",
        length: 4,
        revision: 1,
        notes: [{ id: "note-1", start: 0, duration: 1, pitch: 128, velocity: 0.8 }],
      }],
    };
    const nonfinite = {
      ...project,
      definitions: [{
        id: "definition-1",
        name: "Invalid pitch",
        length: 4,
        revision: 1,
        notes: [{ id: "note-1", start: 0, duration: 1, pitch: Infinity, velocity: 0.8 }],
      }],
    };

    expect(ProjectSchema.safeParse(project).success).toBe(true);
    expect(ProjectSchema.safeParse(nonfinite).success).toBe(false);
  });

  it("preserves identity-based note patches returned by Rust", () => {
    const patch = { type: "patchNotes", updates: [{ id: "n1", start: 0, duration: 1, pitch: 61, velocity: 0.8 }], added: [], removed: ["n2"] };
    const parsed = OperationSchema.parse(patch);
    expect(parsed).toEqual(patch);
  });

  it("rejects internal note patches in reusable modifiers and proposal hunks", () => {
    const patch = { type: "patchNotes", updates: [], added: [], removed: [] };
    const modifier = ModifierSchema.safeParse({ id: "m1", name: "Local only", operations: [patch] });
    const hunk = HunkSchema.safeParse({ id: "h1", title: "Local only", description: "", operation: patch, dependencies: [], accepted: false });
    expect(modifier.success).toBe(false);
    expect(hunk.success).toBe(false);
  });

  it("parses a canonical blank project when the API returns the contract shape", () => {
    // Given a project-shaped API payload
    // When the payload crosses the Zod boundary
    const result = ProjectSchema.safeParse(blankProject);

    // Then the canonical project is accepted
    expect(result.success).toBe(true);
  });

  it("accepts canonical tempo and meter maps when scalars match their first points", () => {
    // Given
    const mapped = {
      ...blankProject,
      meter: [3, 8],
      tempoMap: { points: [{ at: 0, bpm: 124, curve: "linear" }, { at: 4, bpm: 100, curve: "step" }] },
      meterMap: { points: [{ at: 0, numerator: 3, denominator: 8 }, { at: 3, numerator: 5, denominator: 4 }] },
    };

    // When
    const result = ProjectSchema.safeParse(mapped);

    // Then
    expect(result.success).toBe(true);
  });

  it("rejects mismatched scalars, off-grid tempo points, and meter changes inside a bar", () => {
    // Given
    const invalid = [
      { ...blankProject, tempoMap: { points: [{ at: 0, bpm: 120, curve: "step" }] } },
      { ...blankProject, tempoMap: { points: [{ at: 0, bpm: 124, curve: "step" }, { at: 1 / 1000, bpm: 120, curve: "step" }] } },
      { ...blankProject, meterMap: { points: [{ at: 0, numerator: 4, denominator: 4 }, { at: 3, numerator: 3, denominator: 4 }] } },
    ];

    // When
    const results = invalid.map((project) => ProjectSchema.safeParse(project).success);

    // Then
    expect(results).toEqual([false, false, false]);
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
