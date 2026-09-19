import {
	Clock3,
	Download,
	FolderOpen,
	Plus,
	ShieldCheck,
	X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { z } from "zod";
import type { ProjectSummary } from "../domain/api";
import type { Project } from "../domain/project";
import { DiagnosticSchema } from "../domain/project";
import { apiErrorMessage, rpc } from "../lib/api-client";
import { Button, IconButton } from "./primitives";

const ValidationSchema = z.object({
	valid: z.boolean(),
	diagnostics: z.array(DiagnosticSchema),
});

type ProjectsDialogProps = {
	readonly project: Project;
	readonly projects: readonly ProjectSummary[];
	readonly open: boolean;
	readonly onClose: () => void;
	readonly onOpen: (id: string) => void;
	readonly onCreate: (name: string, fixture: boolean) => void;
	readonly onExport: () => void;
	readonly onRename: (name: string) => void;
};

export const ProjectsDialog = ({
	project,
	projects,
	open,
	onClose,
	onOpen,
	onCreate,
	onExport,
	onRename,
}: ProjectsDialogProps) => {
	const dialogRef = useRef<HTMLDialogElement>(null);
	const [name, setName] = useState("");
	const [projectName, setProjectName] = useState(project.name);
	const [fixture, setFixture] = useState(true);
	const [validation, setValidation] =
		useState<z.infer<typeof ValidationSchema>>();
	const [validationError, setValidationError] = useState<string>();
	useEffect(() => setProjectName(project.name), [project.name]);
	useEffect(() => {
		const dialog = dialogRef.current;
		if (dialog === null) return;
		if (open && !dialog.open) dialog.showModal();
		if (!open && dialog.open) dialog.close();
	}, [open]);
	useEffect(() => {
		if (!open) return;
		let active = true;
		const requestedRevision = project.revision;
		setValidation(undefined);
		setValidationError(undefined);
		void rpc(
			{ projectId: project.id, method: "project.validate" },
			ValidationSchema,
		)
			.then(({ state, result }) => {
				if (active && state.revision === requestedRevision)
					setValidation(result);
			})
			.catch((error: unknown) => {
				if (active) setValidationError(apiErrorMessage(error));
			});
		return () => {
			active = false;
		};
	}, [open, project.id, project.revision]);

	return (
		<dialog
			ref={dialogRef}
			className="dialog-scrim"
			aria-labelledby="projects-title"
			onCancel={(event) => {
				event.preventDefault();
				onClose();
			}}
			onMouseDown={(event) => {
				if (event.target === event.currentTarget) onClose();
			}}
		>
			<section className="dialog">
				<header>
					<div>
						<span className="eyebrow">Workspace</span>
						<h2 id="projects-title">Projects & history</h2>
					</div>
					<IconButton label="Close projects" icon={X} onClick={onClose} />
				</header>
				<div className="dialog__grid">
					<section>
						<h3>
							<FolderOpen aria-hidden="true" size="var(--icon-sm)" /> Recent
							projects
						</h3>
						<form
							className="rename-project"
							onSubmit={(event) => {
								event.preventDefault();
								if (projectName.trim().length > 0) onRename(projectName.trim());
							}}
						>
							<label>
								<span>Current project name</span>
								<input
									value={projectName}
									onChange={(event) => setProjectName(event.target.value)}
								/>
							</label>
							<Button type="submit" variant="subtle">
								Rename
							</Button>
						</form>
						<div className="project-list">
							{projects.map((item) => (
								<button
									type="button"
									key={item.id}
									data-active={item.id === project.id}
									onClick={() => {
										onOpen(item.id);
										onClose();
									}}
								>
									<span>
										<strong>{item.name}</strong>
										<small>{item.updatedAt}</small>
									</span>
									<em>{item.revision.slice(0, 8)}</em>
								</button>
							))}
						</div>
						<form
							onSubmit={(event) => {
								event.preventDefault();
								if (name.trim().length > 0) onCreate(name.trim(), fixture);
							}}
							className="create-project"
						>
							<label>
								<span>New project name</span>
								<input
									value={name}
									onChange={(event) => setName(event.target.value)}
									placeholder="Untitled set"
								/>
							</label>
							<label className="check-row">
								<input
									type="checkbox"
									checked={fixture}
									onChange={(event) => setFixture(event.target.checked)}
								/>
								Start with shared-chorus fixture
							</label>
							<Button icon={Plus} variant="primary" type="submit">
								Create project
							</Button>
						</form>
					</section>
					<section>
						<h3>
							<ShieldCheck aria-hidden="true" size="var(--icon-sm)" /> Project
							health
						</h3>
						<div className="project-health" role="status">
							{validationError ??
								(validation === undefined
									? "Checking canonical project…"
									: validation.valid
										? `Valid canonical project · ${validation.diagnostics.length} diagnostics`
										: `Validation failed · ${validation.diagnostics.length} diagnostics`)}
						</div>
						{validation?.diagnostics.map((item) => (
							<p
								className="project-health__diagnostic"
								key={`${item.code}-${item.entityId ?? "project"}`}
							>
								{item.severity}: {item.message}
							</p>
						))}
						<h3>
							<Clock3 aria-hidden="true" size="var(--icon-sm)" /> Canonical
							revision history
						</h3>
						<ol className="history-list">
							{[...project.history].reverse().map((entry) => (
								<li key={entry.id}>
									<span />
									<div>
										<strong>{entry.message}</strong>
										<small>{entry.author}</small>
									</div>
									<em>{entry.revision.slice(0, 8)}</em>
								</li>
							))}
						</ol>
					</section>
				</div>
				<footer>
					<Button icon={Download} onClick={onExport}>
						Export canonical JSON
					</Button>
					<Button variant="primary" onClick={onClose}>
						Done
					</Button>
				</footer>
			</section>
		</dialog>
	);
};
