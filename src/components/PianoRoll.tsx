import { Link2, Minus, Plus, ZoomIn } from "lucide-react";
import { useState } from "react";
import type { Instance, Note, Project } from "../domain/project";
import { NoteSchema } from "../domain/project";
import { IconButton, Tabs } from "./primitives";

type EditScope = "source" | "instance";
const scopeTabs = [
	{ value: "source", label: "Source" },
	{ value: "instance", label: "This instance" },
] as const;

type PianoRollProps = {
	readonly project: Project;
	readonly instance: Instance | undefined;
	readonly effectiveNotes: readonly Note[] | undefined;
	readonly onCommit: (scope: EditScope, notes: readonly Note[]) => void;
};

export const PianoRoll = ({
	project, instance, effectiveNotes, onCommit,
}: PianoRollProps) => {
	const [scope, setScope] = useState<EditScope>("instance");
	const [zoom, setZoom] = useState(1);
	const definition = project.definitions.find(
		(item) => item.id === instance?.definitionId,
	);
	const notes = scope === "source" ? definition?.notes ?? [] : effectiveNotes ?? [];
	const highest = notes.reduce(
		(value, note) => Math.max(value, note.pitch), notes[0]?.pitch ?? 72,
	);
	const lowest = notes.reduce(
		(value, note) => Math.min(value, note.pitch), notes[0]?.pitch ?? 60,
	);
	const topPitch = Math.ceil(highest / 12) * 12;
	const pitchRows = Array.from(
		{ length: Math.max(13, Math.min(128, topPitch - Math.floor(lowest) + 1)) },
		(_, index) => topPitch - index,
	);
	const length = definition?.length ?? 8;
	const commitAt = (start: number, pitch: number) => {
		if (instance === undefined || definition === undefined) return;
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
		onCommit(scope, next);
	};

	const edit = (event: React.MouseEvent<HTMLButtonElement>) => {
		if (instance === undefined || definition === undefined) return;
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
					<strong>{instance?.name ?? "No clip selected"}</strong>
					{definition === undefined ? null : (
						<span>
							<Link2 aria-hidden="true" size="var(--icon-sm)" /> Linked to{" "}
							{definition.name}
						</span>
					)}
				</div>
				<Tabs
					value={scope}
					items={scopeTabs}
					onChange={setScope}
					label="Note editing scope"
				/>
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
					disabled={instance === undefined || (scope === "instance" && effectiveNotes === undefined)}
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
					</svg>
				</button>
			</div>
			<footer className="editor-footer">
				<span>
					{scope === "instance" && effectiveNotes === undefined
						? "Resolving canonical instance notes…"
						: scope === "source"
						? "Edits update every linked instance."
						: "Edits create an override for this instance only."}
				</span>
				<span>
					Grid 1/16 · Velocity {Math.round((notes.at(-1)?.velocity ?? 0) * 100)}
					%
				</span>
			</footer>
		</section>
	);
};
