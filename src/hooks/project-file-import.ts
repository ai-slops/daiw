import type { Project } from "../domain/project";

type ProjectImporter = (document: unknown) => Promise<Project>;

export class ProjectFileError extends Error {
	readonly name = "ProjectFileError";
}

export const importProjectFile = async (
	file: File,
	importDocument: ProjectImporter,
): Promise<Project> => {
	let document: unknown;
	try {
		document = JSON.parse(await file.text());
	} catch (error) {
		if (error instanceof SyntaxError) {
			throw new ProjectFileError("The selected file is not valid JSON.", {
				cause: error,
			});
		}
		throw error;
	}
	return importDocument(document);
};
