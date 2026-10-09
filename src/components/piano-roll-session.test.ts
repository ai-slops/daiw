import { describe, expect, it } from "vitest";
import { CompositionNoteSchema } from "../domain/api";
import { DefinitionChildSchema, DefinitionSchema, InstanceSchema, NoteSchema } from "../domain/project";
import {
	addDefinitionChildCommand,
	definitionChildCandidates,
	definitionPathSessionKey,
	editDefinitionNotesCommand,
	parseDefinitionChildDraft,
	pianoRollSessionKey,
	provenanceLabel,
	removeDefinitionChildCommand,
	setDefinitionChildCommand,
	settleDefinitionChildDraft,
	sourceNoteLayers,
} from "./piano-roll-session";

const instance = InstanceSchema.parse({
	id: "instance-a",
	name: "A",
	definitionId: "definition-a",
	trackId: "track-a",
	start: 0,
	length: 4,
	modifiers: [],
	overrides: [],
});

describe("piano-roll editor isolation", () => {
	it("uses a distinct session when the selected instance changes", () => {
		// Given two clips that share one source definition
		const sibling = InstanceSchema.parse({ ...instance, id: "instance-b" });

		// When their editor identities are derived
		const firstKey = pianoRollSessionKey(instance);
		const siblingKey = pianoRollSessionKey(sibling);

		// Then a draft-capable editor cannot reuse local state across clips
		expect(siblingKey).not.toBe(firstKey);
	});

	it("uses a distinct session when the source definition changes", () => {
		// Given one placement rebound to another definition
		const rebound = InstanceSchema.parse({
			...instance,
			definitionId: "definition-b",
		});

		// When their editor identities are derived
		const firstKey = pianoRollSessionKey(instance);
		const reboundKey = pianoRollSessionKey(rebound);

		// Then stale source-local state cannot cross the definition boundary
		expect(reboundKey).not.toBe(firstKey);
	});
});

describe("nested definition editor state", () => {
	it("rejects empty, off-grid, and out-of-range placement drafts with visible validation results", () => {
		// Given
		const drafts = [
			{ definitionId: "leaf", start: "", length: "4" },
			{ definitionId: "leaf", start: "0.1", length: "4" },
			{ definitionId: "leaf", start: "7", length: "4" },
		];

		// When
		const results = drafts.map((draft) => parseDefinitionChildDraft(draft, 8));

		// Then
		expect(results.map((result) => result.kind)).toEqual(["invalid", "invalid", "invalid"]);
		expect(results.map((result) => result.kind === "invalid" ? result.invalidFields : [])).toEqual([
			["start"],
			["start"],
			["start", "length"],
		]);
		expect(results[0]).toMatchObject({ message: "Start is required." });
	});

	it("preserves a placement draft after failure and clears it only after success", () => {
		// Given
		const draft = { definitionId: "leaf", start: "2", length: "4" };

		// When
		const afterFailure = settleDefinitionChildDraft(draft, false);
		const afterSuccess = settleDefinitionChildDraft(draft, true);

		// Then
		expect(afterFailure).toBe(draft);
		expect(afterSuccess).toBeUndefined();
	});

	it("changes the definition path session when projects reuse instance identifiers", () => {
		// Given
		const siblingProjectInstance = InstanceSchema.parse({ ...instance });

		// When
		const first = definitionPathSessionKey("project-a", instance);
		const second = definitionPathSessionKey("project-b", siblingProjectInstance);

		// Then
		expect(second).not.toBe(first);
	});

	it("keeps source-owned notes solid and nested composition notes ghosted", () => {
		// Given
		const own = [NoteSchema.parse({ id: "root-note", start: 0, duration: 1, pitch: 60, velocity: 0.8 })];
		const composition = [
			CompositionNoteSchema.parse({ id: "render-root", start: 0, duration: 1, pitch: 60, velocity: 0.8, origin: { definitionId: "root", noteId: "root-note", modifierIds: [] } }),
			CompositionNoteSchema.parse({ id: "render-leaf", start: 2, duration: 1, pitch: 67, velocity: 0.7, origin: { definitionId: "leaf", noteId: "leaf-note", modifierIds: [], definitionPath: [{ definitionId: "root", definitionRevision: 1 }, { definitionId: "leaf", definitionRevision: 2, placementId: "child-1" }] } }),
		];

		// When
		const layers = sourceNoteLayers("root", own, composition);

		// Then
		expect(layers.solid).toEqual(own);
		expect(layers.ghosts.map((note) => note.id)).toEqual(["render-leaf"]);
	});

	it("formats leaf provenance from the complete definition path", () => {
		// Given
		const note = CompositionNoteSchema.parse({
			id: "render-leaf", start: 2, duration: 1, pitch: 67, velocity: 0.7,
			origin: {
				definitionId: "leaf", noteId: "leaf-note", modifierIds: ["humanize"],
				definitionPath: [{ definitionId: "root", definitionRevision: 1 }, { definitionId: "leaf", definitionRevision: 2, placementId: "child-1", loopIndex: 3 }],
				instanceLoopIndex: 2,
			},
		});

		// When
		const label = provenanceLabel(note, new Map([["root", "Song"], ["leaf", "Fill"]]));

		// Then
		expect(label).toBe("Song r1 / Fill r2 · placement child-1 · loop 3 · instance loop 2 · modifiers humanize · note leaf-note");
	});

	it("omits the current parent and descendants from child choices", () => {
		// Given
		const definitions = [
			{ id: "root", name: "Root", length: 8, revision: 1, notes: [], children: [{ id: "child-a", definitionId: "middle", start: 0, length: 8 }] },
			{ id: "middle", name: "Middle", length: 8, revision: 1, notes: [], children: [{ id: "child-b", definitionId: "leaf", start: 0, length: 4 }] },
			{ id: "leaf", name: "Leaf", length: 4, revision: 1, notes: [] },
			{ id: "other", name: "Other", length: 4, revision: 1, notes: [] },
		].map((definition) => DefinitionSchema.parse(definition));

		// When
		const candidates = definitionChildCandidates(definitions, "leaf");

		// Then
		expect(candidates.map((definition) => definition.id)).toEqual(["other"]);
	});

	it("builds exact add, set, remove, and own-note command payloads", () => {
		// Given
		const child = DefinitionChildSchema.parse({ id: "child-1", definitionId: "leaf", start: 2, length: 4 });

		// When / Then
		expect(addDefinitionChildCommand("root", child)).toEqual({ type: "addDefinitionChild", parentDefinitionId: "root", definitionId: "leaf", start: 2, length: 4 });
		expect(setDefinitionChildCommand("root", child)).toEqual({ type: "setDefinitionChild", parentDefinitionId: "root", child });
		expect(removeDefinitionChildCommand("root", "child-1")).toEqual({ type: "removeDefinitionChild", parentDefinitionId: "root", childId: "child-1" });
		expect(editDefinitionNotesCommand("root", [])).toEqual({ type: "editDefinitionNotes", definitionId: "root", notes: [] });
	});
});
