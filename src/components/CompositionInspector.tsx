import { ChevronRight, CornerDownRight, Pencil, Plus, Trash2, X } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { Definition, DefinitionChild, Project } from "../domain/project";
import {
	definitionChildCandidates,
	parseDefinitionChildDraft,
	settleDefinitionChildDraft,
	type DefinitionChildDraft,
	type DefinitionChildDraftError,
} from "./piano-roll-session";
import { Button, IconButton } from "./primitives";

type CompositionInspectorProps = {
	readonly project: Project;
	readonly path: readonly string[];
	readonly busy: boolean;
	readonly error: string | undefined;
	readonly onClearError: () => void;
	readonly onSelectParent: (definitionId: string) => void;
	readonly onNavigateChild: (definitionId: string) => void;
	readonly onNavigateCrumb: (index: number) => void;
	readonly onAdd: (parentDefinitionId: string, child: DefinitionChild) => Promise<boolean>;
	readonly onSet: (parentDefinitionId: string, child: DefinitionChild) => Promise<boolean>;
	readonly onRemove: (parentDefinitionId: string, childId: string) => void;
};

const definitionName = (definitions: readonly Definition[], definitionId: string) =>
	definitions.find((definition) => definition.id === definitionId)?.name ?? definitionId;

export const CompositionInspector = ({
	project,
	path,
	busy,
	error,
	onClearError,
	onSelectParent,
	onNavigateChild,
	onNavigateCrumb,
	onAdd,
	onSet,
	onRemove,
}: CompositionInspectorProps) => {
	const titleId = useId();
	const errorId = useId();
	const parentId = path.at(-1) ?? project.definitions[0]?.id;
	const parent = project.definitions.find((definition) => definition.id === parentId);
	const [draft, setDraft] = useState<DefinitionChildDraft>();
	const [validationError, setValidationError] = useState<DefinitionChildDraftError>();
	const [savePending, setSavePending] = useState(false);
	useEffect(() => {
		setDraft(undefined);
		setValidationError(undefined);
	}, [parentId]);
	if (parent === undefined) return null;
	const candidates = definitionChildCandidates(project.definitions, parent.id);
	const candidateIds = new Set(candidates.map((definition) => definition.id));
	const saving = busy || savePending;
	const draftError = validationError?.message ?? error;
	const invalidFields = new Set(validationError?.invalidFields ?? []);
	const describedBy = draftError === undefined ? undefined : errorId;
	const clearDraftError = () => {
		setValidationError(undefined);
		onClearError();
	};
	const saveDraft = async () => {
		if (draft === undefined || saving) return;
		const parsed = parseDefinitionChildDraft(draft, parent.length);
		if (parsed.kind === "invalid") {
			setValidationError(parsed);
			return;
		}
		setValidationError(undefined);
		setSavePending(true);
		const succeeded = draft.childId === undefined
			? await onAdd(parent.id, parsed.child)
			: await onSet(parent.id, parsed.child);
		setSavePending(false);
		setDraft(settleDefinitionChildDraft(draft, succeeded));
	};

	return (
		<section className="composition-section" aria-labelledby={titleId}>
			<div className="section-label">
				<span id={titleId}>COMPOSITION</span>
				<small>{parent.children?.length ?? 0} children inside this source</small>
			</div>
			<label className="composition-parent">
				<span>Parent definition</span>
				<select value={path[0] ?? parent.id} disabled={saving} onChange={(event) => onSelectParent(event.target.value)}>
					{project.definitions.map((definition) => <option key={definition.id} value={definition.id}>{definition.name}</option>)}
				</select>
			</label>
			<nav className="composition-breadcrumb" aria-label="Definition path">
				{path.map((definitionId, index) => (
					<span key={`${definitionId}-${index}`}>
						{index === 0 ? null : <ChevronRight aria-hidden="true" size="var(--icon-sm)" />}
						<button type="button" disabled={saving || index === path.length - 1} onClick={() => onNavigateCrumb(index)}>
							{definitionName(project.definitions, definitionId)}
						</button>
					</span>
				))}
			</nav>
			<p className="composition-context">Editing <strong>{parent.name}</strong> own notes and child placements. Nested notes stay linked and are never flattened into this source.</p>
			<div className="composition-list">
				{(parent.children ?? []).map((child) => {
					const childDefinition = project.definitions.find((definition) => definition.id === child.definitionId);
					return (
						<div className="composition-child" key={child.id}>
							<button className="composition-child__open" type="button" disabled={saving} onClick={() => onNavigateChild(child.definitionId)}>
								<CornerDownRight aria-hidden="true" size="var(--icon-sm)" />
								<span><strong>{childDefinition?.name ?? child.definitionId}</strong><small>{child.start.toFixed(2)} → {(child.start + child.length).toFixed(2)} beats</small></span>
							</button>
							<IconButton label={`Edit ${childDefinition?.name ?? "child"}`} icon={Pencil} disabled={saving} onClick={() => { clearDraftError(); setDraft({ childId: child.id, definitionId: child.definitionId, start: String(child.start), length: String(child.length) }); }} />
							<IconButton label={`Remove ${childDefinition?.name ?? "child"}`} icon={Trash2} variant="danger" disabled={saving} onClick={() => onRemove(parent.id, child.id)} />
						</div>
					);
				})}
				{(parent.children?.length ?? 0) === 0 ? <p className="quiet-copy">No child placements. This source currently contains only its own notes.</p> : null}
			</div>
			{draft === undefined ? (
				<Button icon={Plus} variant="ghost" disabled={saving || candidates.length === 0} onClick={() => {
					const first = candidates[0];
					if (first !== undefined) { clearDraftError(); setDraft({ definitionId: first.id, start: "0", length: String(Math.min(first.length, parent.length)) }); }
				}}>Add child</Button>
			) : (
				<div className="composition-draft">
					<label><span>Source</span><select value={draft.definitionId} disabled={saving} aria-invalid={invalidFields.has("definitionId")} aria-describedby={describedBy} onChange={(event) => { clearDraftError(); setDraft({ ...draft, definitionId: event.target.value }); }}>
						{project.definitions.filter((definition) => candidateIds.has(definition.id) || definition.id === draft.definitionId).map((definition) => <option key={definition.id} value={definition.id}>{definition.name}</option>)}
					</select></label>
					<label><span>Start</span><input type="number" min="0" max={parent.length} step="0.25" value={draft.start} disabled={saving} aria-invalid={invalidFields.has("start")} aria-describedby={describedBy} onChange={(event) => { clearDraftError(); setDraft({ ...draft, start: event.target.value }); }} /></label>
					<label><span>Length</span><input type="number" min="0.25" max={parent.length} step="0.25" value={draft.length} disabled={saving} aria-invalid={invalidFields.has("length")} aria-describedby={describedBy} onChange={(event) => { clearDraftError(); setDraft({ ...draft, length: event.target.value }); }} /></label>
					<div className="composition-draft__actions"><Button variant="primary" disabled={saving} onClick={() => void saveDraft()}>{draft.childId === undefined ? "Add placement" : "Save placement"}</Button><IconButton label="Cancel placement edit" icon={X} disabled={saving} onClick={() => { clearDraftError(); setDraft(undefined); }} /></div>
				</div>
			)}
			{draftError === undefined ? <p className="composition-validation">Parent range and nesting depth are validated by the core.</p> : null}
			{saving ? <p className="composition-status" role="status">Saving composition…</p> : null}
			{draftError === undefined ? null : <p id={errorId} className="composition-error" role="alert">{draftError}</p>}
		</section>
	);
};
