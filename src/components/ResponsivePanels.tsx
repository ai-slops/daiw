import type { Instance, Project } from "../domain/project";
import type { useStudio } from "../hooks/use-studio";
import { BrowserPanel } from "./BrowserPanel";
import { Inspector } from "./Inspector";

type ResponsivePanelProps = {
	readonly project: Project;
	readonly selected: Instance | undefined;
	readonly studio: ReturnType<typeof useStudio>;
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
}: ResponsivePanelProps) => (
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
	/>
);
