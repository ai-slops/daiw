import { AlertTriangle, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Arrangement } from "./components/Arrangement";
import { Mixer } from "./components/Mixer";
import { PianoRoll } from "./components/PianoRoll";
import { ProjectsDialog } from "./components/ProjectsDialog";
import { ProposalPanel } from "./components/ProposalPanel";
import { pianoRollSessionKey } from "./components/piano-roll-session";
import { IconButton } from "./components/primitives";
import {
	BrowserWorkspace,
	InspectorWorkspace,
} from "./components/ResponsivePanels";
import { StudioRail, type Workspace } from "./components/StudioRail";
import { TopBar } from "./components/TopBar";
import { Transport } from "./components/Transport";
import type { Note } from "./domain/project";
import { useAudio } from "./hooks/use-audio";
import { useStudio } from "./hooks/use-studio";
import { downloadMix, downloadProject } from "./lib/api-client";

export const App = () => {
	const studio = useStudio();
	const project = studio.project;

	if (project === undefined) {
		return (
			<main className="loading-screen">
				<span className="brand-mark__signal" />
				<h1>DAIW / Studio</h1>
				<p>{studio.error ?? "Opening the audio engine…"}</p>
				{studio.error === undefined ? null : (
					<button type="button" onClick={() => void studio.refreshList()}>
						Retry
					</button>
				)}
			</main>
		);
	}

	return <LoadedStudio studio={studio} project={project} />;
};

type LoadedStudioProps = {
	readonly studio: ReturnType<typeof useStudio>;
	readonly project: NonNullable<ReturnType<typeof useStudio>["project"]>;
};

const LoadedStudio = ({ studio, project }: LoadedStudioProps) => {
	const [selectedId, setSelectedId] = useState<string>();
	const [workspace, setWorkspace] = useState<Workspace>("arrangement");
	const [projectsOpen, setProjectsOpen] = useState(false);
	const fileInput = useRef<HTMLInputElement>(null);
	const audio = useAudio(project);
	const selected = useMemo(
		() =>
			project.instances.find((instance) => instance.id === selectedId) ??
			project.instances[0],
		[project, selectedId],
	);
	useEffect(() => {
		if (selected !== undefined) void studio.explain(selected.id);
	}, [selected, studio.explain]);

	const editNotes = (scope: "source" | "instance", notes: readonly Note[]) => {
		if (selected === undefined) return;
		void studio.command({
			type: "editNotes",
			instanceId: selected.id,
			scope,
			notes,
		});
	};
	const exportProject = () => downloadProject(project.id);
	const browserPanel = (
		<BrowserWorkspace project={project} selected={selected} studio={studio} />
	);
	const inspectorPanel = (
		<InspectorWorkspace project={project} selected={selected} studio={studio} />
	);

	return (
		<div className="studio-shell" data-workspace={workspace}>
			<TopBar
				project={project}
				busy={studio.busy}
				healthy={studio.protocolVersion !== undefined}
				hasError={studio.error !== undefined}
				onProjects={() => setProjectsOpen(true)}
				onUndo={() => void studio.history("undo")}
				onRedo={() => void studio.history("redo")}
				onExport={exportProject}
				onImport={() => fileInput.current?.click()}
			/>
			<input
				ref={fileInput}
				className="sr-only"
				type="file"
				accept="audio/*,.wav"
				aria-label="Import audio file"
				onChange={(event) => {
					const file = event.currentTarget.files?.[0];
					if (file !== undefined) void studio.upload(file);
					event.currentTarget.value = "";
				}}
			/>
			<StudioRail
				workspace={workspace}
				onWorkspace={setWorkspace}
				onAddTrack={() =>
					void studio.command({
						type: "addTrack",
						name: `Track ${project.tracks.length + 1}`,
						instrument: "synth",
						color: "amber",
						role: "instrument",
					})
				}
				onAddDefinition={() =>
					void studio.command({
						type: "addDefinition",
						name: `Source ${project.definitions.length + 1}`,
						length: 8,
						notes: [],
					})
				}
			/>
			<aside className="browser-slot">{browserPanel}</aside>
			<main className="studio-main" id="studio-main">
				<Transport
					tempo={project.tempo}
					onTempo={(tempo) =>
						Number.isFinite(tempo) &&
						void studio.command({ type: "setTempo", tempo })
					}
					audio={audio.state}
					busy={audio.busy}
					onToggle={() => void audio.toggle()}
					onStop={audio.stop}
					onSeek={audio.seek}
					onLoop={audio.toggleLoop}
					onRender={() => downloadMix(project.id)}
				/>
				{workspace === "arrangement" ? (
					<>
						<Arrangement
							project={project}
							positionBeat={audio.state.positionBeat}
							selectedInstanceId={selected?.id}
							onSelect={setSelectedId}
							onDuplicate={(instanceId) =>
								void studio.command({ type: "duplicateInstance", instanceId })
							}
							onDelete={(instanceId) =>
								void studio.command({ type: "deleteInstance", instanceId })
							}
						/>
						<PianoRoll
							key={pianoRollSessionKey(selected)}
							project={project}
							instance={selected}
							effectiveNotes={
								studio.explanation?.instance.id === selected?.id &&
								studio.explanation?.revision === project.revision
									? studio.explanation.effectiveNotes
									: undefined
							}
							onCommit={editNotes}
						/>
					</>
				) : null}
				{workspace === "mixer" ? (
					<Mixer
						tracks={project.tracks}
						meter={audio.meter}
						onTrack={(trackId, patch) =>
							void studio.command({ type: "setTrack", trackId, ...patch })
						}
					/>
				) : null}
				{workspace === "assistant" ? (
					<ProposalPanel
						instance={selected}
						proposals={project.proposals}
						busy={studio.busy || audio.busy}
						onCreate={(request) =>
							selected === undefined
								? undefined
								: void studio.createProposal(selected.id, request)
						}
						onAccept={(proposalId, hunkIds) =>
							void studio.decideProposal(proposalId, hunkIds)
						}
						onReject={(proposalId) => void studio.decideProposal(proposalId)}
						audio={audio.state}
						onPrepareAB={(proposalId) => void audio.prepareAB(proposalId)}
						onSelectAB={audio.selectAB}
						onLoudnessMatch={audio.setLoudnessMatching}
					/>
				) : null}
				{workspace === "browser" ? browserPanel : null}
				{workspace === "inspector" ? inspectorPanel : null}
			</main>
			<aside className="inspector-slot">{inspectorPanel}</aside>
			{studio.error === undefined || projectsOpen ? null : (
				<div className="error-toast" role="alert">
					<AlertTriangle aria-hidden="true" />
					<span>{studio.error}</span>
					<IconButton
						label="Dismiss error"
						icon={X}
						onClick={studio.clearError}
					/>
				</div>
			)}
			{audio.error === undefined ? null : (
				<div className="error-toast error-toast--audio" role="alert">
					<AlertTriangle aria-hidden="true" />
					<span>{audio.error}</span>
				</div>
			)}
			<ProjectsDialog
				project={project}
				projects={studio.projects}
				open={projectsOpen}
				onClose={() => setProjectsOpen(false)}
				onOpen={(id) => void studio.openProject(id)}
				onCreate={(name, fixture) => void studio.createProject(name, fixture)}
				onExport={exportProject}
				onImport={studio.importProjectDocument}
				onRename={(name) =>
					void studio.command({ type: "renameProject", name })
				}
				busy={studio.busy}
				error={studio.error}
				onClearError={studio.clearError}
			/>
		</div>
	);
};
