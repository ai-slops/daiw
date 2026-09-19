import type { ProjectSummary } from "../domain/api";
import type { Project } from "../domain/project";

export const updateProjectSummary = (
	summaries: readonly ProjectSummary[],
	project: Project,
): readonly ProjectSummary[] => {
	const updatedAt = new Date().toISOString();
	return summaries.map((summary) =>
		summary.id === project.id
			? {
					...summary,
					name: project.name,
					revision: project.revision,
					updatedAt,
				}
			: summary,
	);
};
