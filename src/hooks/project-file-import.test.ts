import { describe, expect, it, vi } from "vitest";
import { ProjectSchema } from "../domain/project";
import { importProjectFile, ProjectFileError } from "./project-file-import";

const importedProject = ProjectSchema.parse({
	id: "imported-project",
	name: "Imported Project",
	revision: "rev-imported",
	tempo: 120,
	meter: [4, 4],
	definitions: [],
	instances: [],
	tracks: [],
	modifiers: [],
	proposals: [],
	history: [],
});

describe("importProjectFile", () => {
	it("passes the complete native envelope to the importer when the file contains valid JSON", async () => {
		// Given a versioned file with a semantic field unknown to this UI
		const document = {
			format: "daiw-project",
			formatVersion: 1,
			requiredFeatures: ["core-v1"],
			contentHash: `blake3:${"0".repeat(64)}`,
			content: { id: "source-project", futureSemanticField: { mode: "new" } },
		};
		const importer = vi.fn(async () => importedProject);
		const file = new File([JSON.stringify(document)], "session.daiw.json", {
			type: "application/json",
		});

		// When the user imports the file
		const project = await importProjectFile(file, importer);

		// Then no client schema normalizes the document before the core receives it
		expect(importer).toHaveBeenCalledWith(document);
		expect(project).toBe(importedProject);
	});

	it("reports invalid JSON without calling the importer", async () => {
		// Given a file that is not JSON
		const importer = vi.fn(async () => importedProject);
		const file = new File(["not-json"], "broken.json", {
			type: "application/json",
		});

		// When and then the user imports the file
		await expect(importProjectFile(file, importer)).rejects.toEqual(
			new ProjectFileError("The selected file is not valid JSON."),
		);
		expect(importer).not.toHaveBeenCalled();
	});
});
