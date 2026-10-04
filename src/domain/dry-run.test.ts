import { describe, expect, it } from "vitest";
import { DryRunResultSchema } from "./api";

const legacy = {
  executable: true,
  affectedInstances: ["instance-a"],
  hunks: [],
  diagnostics: [],
};
const enriched = {
  ...legacy,
  impact: {
    baseRevision: "rev-1",
    affectedDefinitions: ["definition-a"],
    affectedTracks: ["track-a"],
    previewRanges: [{ instanceId: "instance-a", before: { start: 0, end: 8 }, after: { start: 0, end: 8 } }],
  },
  noteResolution: {
    ready: false,
    instances: [{
      instanceId: "instance-a",
      before: { status: "clean", definitionId: "definition-a", definitionRevision: 1, noteCount: 4 },
      after: {
        status: "conflict", definitionId: "definition-a", definitionRevision: 2,
        diagnostic: { code: "RebaseConflict", severity: "blocking", message: "Missing target", entityId: "instance-a" },
        target: { kind: "missing", noteId: "note-a" },
        actions: ["restoreMissingNote", "editInstanceNotes"],
      },
    }],
  },
};

describe("dry-run compatibility", () => {
  it("preserves structured impact and note resolution from the core", () => {
    const parsed = DryRunResultSchema.parse(enriched);
    expect(parsed).toEqual(enriched);
  });

  it("still accepts an older core response without optional reports", () => {
    expect(DryRunResultSchema.parse(legacy)).toEqual(legacy);
  });

  it("rejects unknown resolution states instead of stripping the report", () => {
    const result = DryRunResultSchema.safeParse({
      ...legacy,
      noteResolution: { ready: true, instances: [{ instanceId: "instance-a", before: { status: "guessed" }, after: { status: "absent" } }] },
    });
    expect(result.success).toBe(false);
  });
});
