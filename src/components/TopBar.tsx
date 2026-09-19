import {
	AlertTriangle,
	Check,
	Download,
	FolderOpen,
	HeartPulse,
	Redo2,
	Undo2,
	Upload,
} from "lucide-react";
import type { Project } from "../domain/project";
import { Button, IconButton } from "./primitives";

type TopBarProps = {
	readonly project: Project;
	readonly busy: boolean;
	readonly healthy: boolean;
	readonly hasError: boolean;
	readonly onProjects: () => void;
	readonly onUndo: () => void;
	readonly onRedo: () => void;
	readonly onExport: () => void;
	readonly onImport: () => void;
};

export const TopBar = ({
	project,
	busy,
	healthy,
	hasError,
	onProjects,
	onUndo,
	onRedo,
	onExport,
	onImport,
}: TopBarProps) => (
	<header className="topbar">
		<div className="brand-mark">
			<span className="brand-mark__signal" aria-hidden="true" />
			<strong>DAIW</strong>
			<span>/ Studio</span>
		</div>
		<button type="button" className="project-title" onClick={onProjects}>
			<span>{project.name}</span>
			<small>rev {project.revision.slice(0, 8)}</small>
		</button>
		<div className="topbar__status" aria-live="polite">
			<HeartPulse aria-hidden="true" size="var(--icon-sm)" />
			<span>{healthy ? "Engine online" : "Engine unavailable"}</span>
		</div>
		<div className="topbar__actions">
			<IconButton
				label="Undo last change"
				icon={Undo2}
				onClick={onUndo}
				disabled={busy}
			/>
			<IconButton
				label="Redo change"
				icon={Redo2}
				onClick={onRedo}
				disabled={busy}
			/>
			<Button icon={Upload} variant="ghost" onClick={onImport}>
				Import
			</Button>
			<Button icon={Download} variant="ghost" onClick={onExport}>
				Export
			</Button>
			<Button icon={FolderOpen} variant="ghost" onClick={onProjects}>
				Projects
			</Button>
			<span
				className="save-state"
				data-state={hasError ? "error" : busy ? "working" : "saved"}
			>
				{hasError ? (
					<AlertTriangle aria-hidden="true" size="var(--icon-sm)" />
				) : (
					<Check aria-hidden="true" size="var(--icon-sm)" />
				)}
				{hasError ? "Needs attention" : busy ? "Working" : "Saved"}
			</span>
		</div>
	</header>
);
