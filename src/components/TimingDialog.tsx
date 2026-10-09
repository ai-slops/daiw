import { X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Project } from "../domain/project";
import { MeterMapEditor } from "./MeterMapEditor";
import { Button, IconButton } from "./primitives";
import { TempoMapEditor } from "./TempoMapEditor";
import {
	beginTimingSave,
	nextTimingSession,
	settleTimingSave,
	type TimingSaveState,
} from "./timing-dialog-state";

type TimingDialogProps = {
	readonly project: Pick<
		Project,
		"revision" | "tempo" | "meter" | "tempoMap" | "meterMap"
	>;
	readonly open: boolean;
	readonly busy: boolean;
	readonly error: string | undefined;
	readonly onClose: () => void;
	readonly onTempoMap: (
		tempoMap: NonNullable<Project["tempoMap"]>,
	) => Promise<unknown>;
	readonly onMeterMap: (
		meterMap: NonNullable<Project["meterMap"]>,
	) => Promise<unknown>;
	readonly onTempo: (tempo: number) => Promise<unknown>;
	readonly onMeter: (meter: Project["meter"]) => Promise<unknown>;
};

export const TimingDialog = ({
	project,
	open,
	busy,
	error,
	onClose,
	onTempoMap,
	onMeterMap,
	onTempo,
	onMeter,
}: TimingDialogProps) => {
	const dialogRef = useRef<HTMLDialogElement>(null);
	const previousOpenRef = useRef(false);
	const [session, setSession] = useState(0);
	const [saveState, setSaveState] = useState<TimingSaveState>({ kind: "idle" });

	useEffect(() => {
		const dialog = dialogRef.current;
		if (dialog === null) return;
		if (open && !dialog.open) dialog.showModal();
		if (!open && dialog.open) dialog.close();
	}, [open]);
	useLayoutEffect(() => {
		setSession((current) =>
			nextTimingSession(current, previousOpenRef.current, open),
		);
		if (open && !previousOpenRef.current) setSaveState({ kind: "idle" });
		previousOpenRef.current = open;
	}, [open]);
	useEffect(() => {
		setSaveState((current) =>
			settleTimingSave(current, {
				busy,
				error,
				revision: project.revision,
			}),
		);
	}, [busy, error, project.revision]);

	const saving = busy || saveState.kind === "pending";
	const save = (job: () => Promise<unknown>) => {
		if (saving) return;
		setSaveState(beginTimingSave(project.revision));
		void job();
	};

	return (
		<dialog
			ref={dialogRef}
			className="dialog-scrim"
			aria-labelledby="timing-title"
			onCancel={(event) => {
				event.preventDefault();
				if (!saving) onClose();
			}}
			onMouseDown={(event) => {
				if (!saving && event.target === event.currentTarget) onClose();
			}}
		>
			<section className="dialog timing-dialog">
				<header>
					<div>
						<span className="eyebrow">Project timing</span>
						<h2 id="timing-title">Tempo & meter maps</h2>
					</div>
					<IconButton
						label="Close timing editor"
						icon={X}
						onClick={onClose}
						disabled={saving}
					/>
				</header>
				<div className="dialog__grid timing-dialog__grid">
					<TempoMapEditor
						key={`tempo-${session}`}
						tempo={project.tempo}
						tempoMap={project.tempoMap}
						onMap={(value) => save(() => onTempoMap(value))}
						onConstant={(value) => save(() => onTempo(value))}
						disabled={saving}
					/>
					<MeterMapEditor
						key={`meter-${session}`}
						meter={project.meter}
						meterMap={project.meterMap}
						onMap={(value) => save(() => onMeterMap(value))}
						onConstant={(value) => save(() => onMeter(value))}
						disabled={saving}
					/>
				</div>
				<footer>
					<div className="timing-save-status" aria-live="polite">
						{saveState.kind === "pending" ? "Saving timing change…" : null}
						{saveState.kind === "saved"
							? `Saved at revision ${saveState.revision}.`
							: null}
						{saveState.kind === "failed" ? (
							<span className="timing-error" role="alert">
								{saveState.message}
							</span>
						) : null}
						{saveState.kind === "idle"
							? "Each map saves independently."
							: null}
					</div>
					<Button onClick={onClose} disabled={saving}>Cancel</Button>
				</footer>
			</section>
		</dialog>
	);
};
