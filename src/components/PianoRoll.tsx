import { Link2, Minus, Plus, ZoomIn } from "lucide-react";
import { useState } from "react";
import type { CompositionNote } from "../domain/api";
import type { Definition, Instance, Note, Project } from "../domain/project";
import { NoteSchema } from "../domain/project";
import { provenanceLabel, sourceNoteLayers } from "./piano-roll-session";
import { IconButton, Tabs } from "./primitives";

type EditScope = "source" | "instance";
const scopeTabs = [
	{ value: "source", label: "Source" },
	{ value: "instance", label: "This instance" },
] as const;

type PianoRollProps = {
	readonly project: Project;
	readonly instance: Instance | undefined;
	readonly sourceDefinition: Definition | undefined;
	readonly sourceParentName: string | undefined;
	readonly effectiveNotes: readonly Note[] | undefined;
	readonly compositionNotes: readonly CompositionNote[] | undefined;
	readonly onCommit: (scope: EditScope, notes: readonly Note[]) => void;
};

export const PianoRoll = ({
	project, instance, sourceDefinition, sourceParentName, effectiveNotes, compositionNotes, onCommit,
}: PianoRollProps) => {
	const [scope, setScope] = useState<EditScope>("instance");
	const [zoom, setZoom] = useState(1);
	const definition = sourceDefinition ?? project.definitions.find((item) => item.id === instance?.definitionId);
	const instanceScopeAvailable = definition?.id === instance?.definitionId;
	const activeScope = instanceScopeAvailable ? scope : "source";
	const layers = sourceNoteLayers(definition?.id ?? "", definition?.notes ?? [], compositionNotes ?? []);
	const notes = activeScope === "source" ? layers.solid : effectiveNotes ?? [];
	const ghosts = activeScope === "source" ? layers.ghosts : [];
	const visibleNotes = [...notes, ...ghosts];
	const definitionNames = new Map<string, string>(project.definitions.map((item) => [item.id, item.name]));
	const firstGhost = ghosts[0];
	const visiblePath = firstGhost?.origin.definitionPath?.map((entry) => definitionNames.get(entry.definitionId) ?? entry.definitionId).join(" / ");
	const compositionPreviewUnavailable = activeScope === "source" && (definition?.children?.length ?? 0) > 0 && compositionNotes === undefined;
	const highest = visibleNotes.reduce(
		(value, note) => Math.max(value, note.pitch), visibleNotes[0]?.pitch ?? 72,
	);
	const lowest = visibleNotes.reduce(
		(value, note) => Math.min(value, note.pitch), visibleNotes[0]?.pitch ?? 60,
	);
	const topPitch = Math.ceil(highest / 12) * 12;
	const pitchRows = Array.from(
		{ length: Math.max(13, Math.min(128, topPitch - Math.floor(lowest) + 1)) },
		(_, index) => topPitch - index,
	);
	const length = definition?.length ?? 8;
	const commitAt = (start: number, pitch: number) => {
		if (definition === undefined || (activeScope === "instance" && instance === undefined)) return;
		const hit = notes.find(
			(note) =>
				Math.abs(note.start - start) < 0.26 &&
				Math.abs(note.pitch - pitch) < 0.5,
		);
		const next =
			hit === undefined
				? [
						...notes,
						NoteSchema.parse({
							id: crypto.randomUUID(),
							start,
							duration: 0.5,
							pitch,
							velocity: 0.75,
						}),
					]
				: notes.filter((note) => note.id !== hit.id);
		onCommit(activeScope, next);
	};

	const edit = (event: React.MouseEvent<HTMLButtonElement>) => {
		if (definition === undefined || (activeScope === "instance" && instance === undefined)) return;
		const bounds = event.currentTarget.getBoundingClientRect();
		const start = Math.max(
			0,
			Math.min(
				length - 0.25,
				Math.round(
					((event.clientX - bounds.left) / bounds.width) * length * 4,
				) / 4,
			),
		);
		const pitchIndex = Math.max(
			0,
			Math.min(
				pitchRows.length - 1,
				Math.floor(
					((event.clientY - bounds.top) / bounds.height) * pitchRows.length,
				),
			),
		);
		const pitch = pitchRows[pitchIndex];
		if (pitch === undefined) return;
		commitAt(start, pitch);
	};

	return (
		<section className="piano-roll" aria-label="Piano roll editor">
			<header className="editor-header">
				<div className="editor-header__title">
					<strong>{activeScope === "source" ? definition?.name ?? "No source selected" : instance?.name ?? "No clip selected"}</strong>
					{definition === undefined ? null : (
						<span>
							<Link2 aria-hidden="true" size="var(--icon-sm)" />
							{activeScope === "instance" ? `Linked to ${definition.name}` : sourceParentName === undefined ? "Shared source · own notes editable" : `Nested in ${sourceParentName} · own notes editable`}
						</span>
					)}
				</div>
				{instanceScopeAvailable ? <Tabs value={activeScope} items={scopeTabs} onChange={setScope} label="Note editing scope" /> : <span className="source-only-badge">Source only</span>}
				<div className="editor-tools">
					<IconButton
						label="Zoom out"
						icon={Minus}
						onClick={() => setZoom((current) => Math.max(0.75, current - 0.25))}
					/>
					<ZoomIn aria-hidden="true" size="var(--icon-sm)" />
					<IconButton
						label="Zoom in"
						icon={Plus}
						onClick={() => setZoom((current) => Math.min(2, current + 0.25))}
					/>
				</div>
			</header>
			<div className="roll-body">
				<div className="piano-keys" aria-hidden="true">
					{pitchRows.map((pitch) => (
						<span
							key={pitch}
							data-black={[1, 3, 6, 8, 10].includes(pitch % 12)}
						>
							{pitch % 12 === 0 ? `C${pitch / 12 - 1}` : ""}
						</span>
					))}
				</div>
				<button
					type="button"
					className="note-grid"
					disabled={definition === undefined || (activeScope === "instance" && (instance === undefined || effectiveNotes === undefined))}
					style={{ minInlineSize: `${600 * zoom}px`, blockSize: `${pitchRows.length * 20}px` }}
					aria-label={`${notes.length} notes. Click or tap the grid to add or remove quantized notes.`}
					onClick={edit}
				>
					<svg
						viewBox={`0 0 ${length * 80} ${pitchRows.length * 20}`}
						preserveAspectRatio="none"
						aria-hidden="true"
					>
						{notes.map((note) => {
							const row = pitchRows.findIndex(
								(pitch) => Math.abs(pitch - note.pitch) < 0.5,
							);
							if (row < 0) return null;
							return (
								<rect
									key={note.id}
									className="midi-note"
									x={note.start * 80}
									y={row * 20 + 2}
									width={Math.max(note.duration * 80 - 3, 8)}
									height="16"
									rx="2"
								/>
							);
						})}
						{ghosts.map((note) => {
							const row = pitchRows.findIndex((pitch) => Math.abs(pitch - note.pitch) < 0.5);
							if (row < 0) return null;
							return <rect key={`ghost-${note.id}`} className="midi-note midi-note--ghost" x={note.start * 80} y={row * 20 + 2} width={Math.max(note.duration * 80 - 3, 8)} height="16" rx="2"><title>{provenanceLabel(note, definitionNames)}</title></rect>;
						})}
					</svg>
				</button>
			</div>
			<footer className="editor-footer">
				<span>
					{activeScope === "instance" && effectiveNotes === undefined
						? "Resolving canonical instance notes…"
						: activeScope === "source"
						? compositionPreviewUnavailable ? "Composed preview unavailable without a scoped instance; own notes remain editable." : ghosts.length === 0 ? "Edits update this definition’s own notes in every linked use." : `${ghosts.length} nested notes are read-only ghosts; edits change only ${definition?.name ?? "this source"} own notes.`
						: "Edits create an override for this instance only."}
				</span>
				<span>
					{firstGhost === undefined ? `Grid 1/16 · Velocity ${Math.round((notes.at(-1)?.velocity ?? 0) * 100)}%` : <span className="provenance-summary" title={provenanceLabel(firstGhost, definitionNames)}>Nested path: {visiblePath ?? firstGhost.origin.definitionId}</span>}
				</span>
			</footer>
		</section>
	);
};
