import type { Instance, Project } from "../domain/project";
import type { useStudio } from "../hooks/use-studio";
import { BrowserPanel } from "./BrowserPanel";
import { Inspector } from "./Inspector";
import {
	addDefinitionChildCommand,
	removeDefinitionChildCommand,
	setDefinitionChildCommand,
} from "./piano-roll-session";

type ResponsivePanelProps = {
	readonly project: Project;
	readonly selected: Instance | undefined;
	readonly studio: ReturnType<typeof useStudio>;
};

type InspectorWorkspaceProps = ResponsivePanelProps & {
	readonly definitionPath: readonly string[];
	readonly onSelectDefinition: (definitionId: string) => void;
	readonly onNavigateChild: (definitionId: string) => void;
	readonly onNavigateCrumb: (index: number) => void;
};

export const BrowserWorkspace = ({
	project,
	selected,
	studio,
}: ResponsivePanelProps) => (
	<BrowserPanel
		project={project}
		selectedInstanceId={selected?.id}
		onApplyModifier={(modifierId) =>
			selected === undefined
				? undefined
				: void studio.command({
						type: "applyModifier",
						instanceId: selected.id,
						modifierId,
					})
		}
		onCreateModifier={() =>
			void studio.command({
				type: "createModifier",
				name: "Lift +3",
				operations: [{ type: "transpose", semitones: 3 }],
			})
		}
		onInstantiate={(definitionId) => {
			const trackId = selected?.trackId ?? project.tracks[0]?.id;
			if (trackId !== undefined)
				void studio.command({
					type: "instantiate",
					definitionId,
					trackId,
					start: project.instances.reduce(
						(end, item) => Math.max(end, item.start + item.length),
						0,
					),
				});
		}}
	/>
);

export const InspectorWorkspace = ({
	project,
	selected,
	studio,
	definitionPath,
	onSelectDefinition,
	onNavigateChild,
	onNavigateCrumb,
}: InspectorWorkspaceProps) => (
	<Inspector
		project={project}
		instance={selected}
		onRemoveModifier={(modifierId) =>
			selected === undefined
				? undefined
				: void studio.command({
						type: "removeModifier",
						instanceId: selected.id,
						modifierId,
					})
		}
		onMove={(start) =>
			selected === undefined
				? undefined
				: void studio.command({
						type: "moveInstance",
						instanceId: selected.id,
						start,
					})
		}
		explanation={studio.explanation}
		dryRun={studio.dryRun}
		onPreviewMove={(start) =>
			selected === undefined
				? undefined
				: void studio.command(
						{ type: "moveInstance", instanceId: selected.id, start },
						true,
					)
		}
		definitionPath={definitionPath}
		busy={studio.busy}
		error={studio.error}
		onClearError={studio.clearError}
		onSelectDefinition={onSelectDefinition}
		onNavigateChild={onNavigateChild}
		onNavigateCrumb={onNavigateCrumb}
		onAddDefinitionChild={(parentDefinitionId, child) => studio.command(addDefinitionChildCommand(parentDefinitionId, child))}
		onSetDefinitionChild={(parentDefinitionId, child) => studio.command(setDefinitionChildCommand(parentDefinitionId, child))}
		onRemoveDefinitionChild={(parentDefinitionId, childId) => void studio.command(removeDefinitionChildCommand(parentDefinitionId, childId))}
	/>
);
