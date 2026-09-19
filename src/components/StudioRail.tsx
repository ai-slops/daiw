import {
	Bot,
	Boxes,
	Library,
	ListMusic,
	PanelRight,
	Plus,
	SlidersHorizontal,
} from "lucide-react";
import { IconButton } from "./primitives";

type Workspace =
	| "arrangement"
	| "mixer"
	| "assistant"
	| "browser"
	| "inspector";
type StudioRailProps = {
	readonly workspace: Workspace;
	readonly onWorkspace: (workspace: Workspace) => void;
	readonly onAddTrack: () => void;
	readonly onAddDefinition: () => void;
};

export const StudioRail = ({
	workspace,
	onWorkspace,
	onAddTrack,
	onAddDefinition,
}: StudioRailProps) => (
	<nav className="studio-rail" aria-label="Studio workspace">
		<div className="studio-rail__main">
			<button
				type="button"
				data-active={workspace === "arrangement"}
				onClick={() => onWorkspace("arrangement")}
			>
				<ListMusic aria-hidden="true" />
				<span>Arrange</span>
			</button>
			<button
				type="button"
				data-active={workspace === "mixer"}
				onClick={() => onWorkspace("mixer")}
			>
				<SlidersHorizontal aria-hidden="true" />
				<span>Mixer</span>
			</button>
			<button
				type="button"
				data-active={workspace === "assistant"}
				onClick={() => onWorkspace("assistant")}
			>
				<Bot aria-hidden="true" />
				<span>Review</span>
			</button>
			<button type="button" onClick={onAddTrack}>
				<Plus aria-hidden="true" />
				<span>Track</span>
			</button>
			<button type="button" onClick={onAddDefinition}>
				<Boxes aria-hidden="true" />
				<span>Source</span>
			</button>
		</div>
		<div className="studio-rail__mobile">
			<button
				type="button"
				data-active={workspace === "browser"}
				onClick={() => onWorkspace("browser")}
			>
				<Library aria-hidden="true" />
				<span>Library</span>
			</button>
			<button
				type="button"
				data-active={workspace === "inspector"}
				onClick={() => onWorkspace("inspector")}
			>
				<PanelRight aria-hidden="true" />
				<span>Details</span>
			</button>
		</div>
		<IconButton
			label="Open library"
			icon={Library}
			onClick={() => onWorkspace("arrangement")}
		/>
	</nav>
);
export type { Workspace };
