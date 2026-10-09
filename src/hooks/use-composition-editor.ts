import { useEffect, useState } from "react";
import type { Instance, Note, Project } from "../domain/project";
import {
	definitionPathSessionKey,
	editDefinitionNotesCommand,
} from "../components/piano-roll-session";
import type { useStudio } from "./use-studio";

type CompositionEditorInput = {
	readonly project: Project;
	readonly selected: Instance | undefined;
	readonly studio: ReturnType<typeof useStudio>;
};

export const useCompositionEditor = ({
	project,
	selected,
	studio,
}: CompositionEditorInput) => {
	const [definitionPath, setDefinitionPath] = useState<readonly string[]>([]);
	const pathSession = definitionPathSessionKey(project.id, selected);
	const fallbackDefinitionId = project.definitions[0]?.id;
	useEffect(() => {
		if (selected !== undefined) void studio.explain(selected.id);
	}, [selected, studio.explain]);
	useEffect(() => {
		const rootDefinitionId = selected?.definitionId ?? fallbackDefinitionId;
		setDefinitionPath(rootDefinitionId === undefined ? [] : [rootDefinitionId]);
	}, [fallbackDefinitionId, pathSession, selected?.definitionId]);
	const sourceDefinitionId = definitionPath.at(-1) ?? selected?.definitionId ?? fallbackDefinitionId;
	const sourceDefinition = project.definitions.find((definition) => definition.id === sourceDefinitionId);
	const sourceParentId = definitionPath.length > 1 ? definitionPath.at(-2) : undefined;
	const sourceParentName = project.definitions.find((definition) => definition.id === sourceParentId)?.name;
	const currentExplanation = studio.explanation?.instance.id === selected?.id && studio.explanation?.revision === project.revision
		? studio.explanation
		: undefined;
	const editNotes = (scope: "source" | "instance", notes: readonly Note[]) => {
		if (scope === "source" && sourceDefinition !== undefined) {
			void studio.command(editDefinitionNotesCommand(sourceDefinition.id, notes));
			return;
		}
		if (selected !== undefined) void studio.command({ type: "editNotes", instanceId: selected.id, scope, notes });
	};
	return {
		definitionPath: definitionPath.length === 0 && sourceDefinitionId !== undefined ? [sourceDefinitionId] : definitionPath,
		sourceDefinition,
		sourceParentName,
		currentExplanation,
		editNotes,
		selectDefinition: (definitionId: string) => setDefinitionPath([definitionId]),
		navigateChild: (definitionId: string) => setDefinitionPath((current) => [...current, definitionId]),
		navigateCrumb: (index: number) => setDefinitionPath((current) => current.slice(0, index + 1)),
	};
};
