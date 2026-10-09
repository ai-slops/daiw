import { useCallback, useEffect, useState } from "react";
import { z } from "zod";
import { DryRunResultSchema, ExplainResultSchema } from "../domain/api";
import type { Operation, Project } from "../domain/project";
import {
	apiErrorMessage,
	getHealth,
	importAudio,
	importProject,
	listProjects,
	rpc,
} from "../lib/api-client";
import { importProjectFile } from "./project-file-import";
import { updateProjectSummary } from "./project-summaries";
import type { StudioState } from "./studio-state";

// allow: SIZE_OK — this hook is the single orchestration boundary for shared Studio state and async jobs.
const EmptyResultSchema = z.record(z.string(), z.unknown());

export const useStudio = () => {
	const [state, setState] = useState<StudioState>({
		project: undefined,
		projects: [],
		busy: true,
		error: undefined,
		dryRun: undefined,
		protocolVersion: undefined,
		explanation: undefined,
	});

	const run = useCallback(
		async <Result>(job: () => Promise<Result>): Promise<Result | undefined> => {
			setState((current) => ({ ...current, busy: true, error: undefined }));
			try {
				return await job();
			} catch (error) {
				setState((current) => ({ ...current, error: apiErrorMessage(error) }));
				return undefined;
			} finally {
				setState((current) => ({ ...current, busy: false }));
			}
		},
		[],
	);

	const refreshList = useCallback(async () => {
		const projects = await run(listProjects);
		if (projects !== undefined)
			setState((current) => ({ ...current, projects }));
	}, [run]);
	const commitProject = useCallback((project: Project) => {
		setState((current) => ({
			...current,
			project,
			projects: updateProjectSummary(current.projects, project),
		}));
	}, []);

	const openProject = useCallback(
		async (projectId: string) => {
			const value = await run(() =>
				rpc({ projectId, method: "project.get" }, EmptyResultSchema),
			);
			if (value !== undefined)
				setState((current) => ({ ...current, project: value.state }));
		},
		[run],
	);

	const createProject = useCallback(
		async (name: string, fixture: boolean) => {
			const value = await run(() =>
				rpc(
					{
						method: "project.create",
						params: { name, fixture },
						mutation: true,
					},
					EmptyResultSchema,
				),
			);
			if (value !== undefined) {
				setState((current) => ({ ...current, project: value.state }));
				await refreshList();
			}
		},
		[refreshList, run],
	);

	useEffect(() => {
		void (async () => {
			const [projects, protocolVersion] = await Promise.all([
				run(listProjects),
				run(getHealth),
			]);
			if (protocolVersion !== undefined)
				setState((current) => ({ ...current, protocolVersion }));
			if (projects === undefined) return;
			setState((current) => ({ ...current, projects }));
			const first = projects[0];
			if (first !== undefined) await openProject(first.id);
			else await createProject("First Light", true);
		})();
	}, [createProject, openProject, run]);

	const command = useCallback(
		async (value: Readonly<Record<string, unknown>>, preview = false): Promise<boolean> => {
			const project = state.project;
			if (project === undefined) return false;
			const method = preview ? "change.dryRun" : "change.apply";
			const schema = preview ? DryRunResultSchema : EmptyResultSchema;
			const response = await run(() =>
				rpc(
					{
						projectId: project.id,
						method,
						params: { expectedRevision: project.revision, command: value },
						mutation: !preview,
						...(preview ? {} : { idempotencyKey: crypto.randomUUID() }),
					},
					schema,
				),
			);
			if (response === undefined) return false;
			if (preview) {
				setState((current) => ({
					...current,
					dryRun: DryRunResultSchema.parse(response.result),
				}));
				return true;
			}
			commitProject(response.state);
			return true;
		},
		[commitProject, run, state.project],
	);

	const createProposal = useCallback(
		async (
			instanceId: string,
			request: string,
			operations?: readonly Operation[],
		) => {
			const project = state.project;
			if (project === undefined) return;
			const response = await run(() =>
				rpc(
					{
						projectId: project.id,
						method: "proposal.create",
						params: {
							expectedRevision: project.revision,
							instanceId,
							request,
							...(operations === undefined ? {} : { operations }),
						},
						mutation: true,
						idempotencyKey: crypto.randomUUID(),
					},
					EmptyResultSchema,
				),
			);
			if (response !== undefined) commitProject(response.state);
		},
		[commitProject, run, state.project],
	);

	const explain = useCallback(
		async (instanceId: string) => {
			const project = state.project;
			if (project === undefined) return;
			const response = await run(() =>
				rpc(
					{
						projectId: project.id,
						method: "project.explain",
						params: { instanceId },
					},
					ExplainResultSchema,
				),
			);
			if (response !== undefined)
				setState((current) => ({ ...current, explanation: response.result }));
		},
		[run, state.project],
	);

	const decideProposal = useCallback(
		async (proposalId: string, hunkIds?: readonly string[]) => {
			const project = state.project;
			if (project === undefined) return;
			const accepted = hunkIds !== undefined;
			const response = await run(() =>
				rpc(
					{
						projectId: project.id,
						method: accepted ? "proposal.accept" : "proposal.reject",
						params: {
							expectedRevision: project.revision,
							proposalId,
							...(accepted ? { hunkIds } : {}),
						},
						mutation: true,
						idempotencyKey: crypto.randomUUID(),
					},
					EmptyResultSchema,
				),
			);
			if (response !== undefined) commitProject(response.state);
		},
		[commitProject, run, state.project],
	);

	const history = useCallback(
		async (direction: "undo" | "redo") => {
			const project = state.project;
			if (project === undefined) return;
			const response = await run(() =>
				rpc(
					{
						projectId: project.id,
						method: `project.${direction}`,
						params: { expectedRevision: project.revision },
						mutation: true,
						idempotencyKey: crypto.randomUUID(),
					},
					EmptyResultSchema,
				),
			);
			if (response !== undefined) commitProject(response.state);
		},
		[commitProject, run, state.project],
	);

	const upload = useCallback(
		async (file: File) => {
			const project = state.project;
			if (project === undefined) return;
			const next = await run(() => importAudio(project, file));
			if (next !== undefined) commitProject(next);
		},
		[commitProject, run, state.project],
	);
	const importProjectDocument = useCallback(
		async (file: File): Promise<boolean> => {
			const next = await run(() => importProjectFile(file, importProject));
			if (next === undefined) return false;
			commitProject(next);
			await refreshList();
			return true;
		},
		[commitProject, refreshList, run],
	);

	return {
		...state,
		createProject,
		openProject,
		refreshList,
		command,
		createProposal,
		decideProposal,
		explain,
		history,
		upload,
		importProjectDocument,
		clearError: () => setState((current) => ({ ...current, error: undefined })),
	};
};
