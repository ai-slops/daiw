import type { DryRunResult, ExplainResult, ProjectSummary } from "../domain/api";
import type { Project } from "../domain/project";

export type StudioState = {
	readonly project: Project | undefined;
	readonly projects: readonly ProjectSummary[];
	readonly busy: boolean;
	readonly error: string | undefined;
	readonly dryRun: DryRunResult | undefined;
	readonly protocolVersion: number | undefined;
	readonly explanation: ExplainResult | undefined;
};
