import { describe, expect, it } from "vitest";
import {
	NoteSelectorSchema,
	QueryNotesParamsSchema,
	SelectionHandleSchema,
	TransformSelectedNotesCommandSchema,
} from "./note-selection";

const selection = {
	projectId: "project-1",
	baseRevision: "rev-1",
	definitionId: "def-1",
	definitionRevision: 2,
	instanceId: "instance-1",
	scope: "source",
	selector: { type: "stableIds", noteIds: ["note-1"] },
	noteIds: ["note-1"],
	cardinality: 1,
	resolutionFingerprint: "hash-1",
};

describe("note selection contract", () => {
	it("parses every bounded non-recursive selector variant", () => {
		const selectors = [
			{ type: "all" },
			{ type: "stableIds", noteIds: ["note-1"] },
			{ type: "namedSlot", slot: "fill" },
			{ type: "beatRange", start: 0, end: 4 },
			{ type: "pitchRange", min: 60, max: 72 },
			{ type: "pitchRange", min: -12.5, max: 140.25 },
		];

		expect(
			selectors.map((selector) => NoteSelectorSchema.parse(selector)),
		).toEqual(selectors);
		expect(
			NoteSelectorSchema.safeParse({ type: "and", selectors }).success,
		).toBe(false);
	});

	it("parses revision-bound selection handles and typed transform commands", () => {
		expect(SelectionHandleSchema.parse(selection)).toEqual(selection);
		const command = {
			type: "transformSelectedNotes",
			selection,
			transform: { type: "setVelocity", velocity: 0.25 },
		};
		expect(TransformSelectedNotesCommandSchema.parse(command)).toEqual(command);
	});

	it("rejects malformed cardinality and unbounded query limits", () => {
		expect(
			SelectionHandleSchema.safeParse({ ...selection, cardinality: -1 })
				.success,
		).toBe(false);
		expect(
			SelectionHandleSchema.safeParse({ ...selection, noteIds: [] }).success,
		).toBe(false);
		expect(
			QueryNotesParamsSchema.safeParse({
				instanceId: "instance-1",
				scope: "source",
				selector: { type: "all" },
				maxResults: 4097,
			}).success,
		).toBe(false);
		expect(
			NoteSelectorSchema.safeParse({
				type: "stableIds",
				noteIds: ["note-1", "note-1"],
			}).success,
		).toBe(false);
	});
});
