import type { CompositionNote } from "../domain/api";
import type { Definition, DefinitionChild, Instance, Note } from "../domain/project";
import { DefinitionChildSchema } from "../domain/project";

export type DefinitionChildDraft = {
	readonly childId?: string;
	readonly definitionId: string;
	readonly start: string;
	readonly length: string;
};

export type DefinitionChildDraftField = "definitionId" | "start" | "length";

export type DefinitionChildDraftError = {
	readonly kind: "invalid";
	readonly message: string;
	readonly invalidFields: readonly DefinitionChildDraftField[];
};

type DefinitionChildDraftResult =
	| { readonly kind: "valid"; readonly child: DefinitionChild }
	| DefinitionChildDraftError;

const placementFieldNames = (fields: readonly DefinitionChildDraftField[]): string =>
	fields.map((field) => field === "definitionId" ? "source" : field).join(" and ").replace(/^./, (letter) => letter.toUpperCase());

export const parseDefinitionChildDraft = (
	draft: DefinitionChildDraft,
	parentLength: number,
): DefinitionChildDraftResult => {
	const emptyFields: DefinitionChildDraftField[] = [
		...(draft.start.trim() === "" ? ["start" as const] : []),
		...(draft.length.trim() === "" ? ["length" as const] : []),
	];
	if (emptyFields.length > 0) {
		return { kind: "invalid", message: `${placementFieldNames(emptyFields)} ${emptyFields.length === 1 ? "is" : "are"} required.`, invalidFields: emptyFields };
	}
	const start = Number(draft.start);
	const length = Number(draft.length);
	const nonFiniteFields: DefinitionChildDraftField[] = [
		...(!Number.isFinite(start) ? ["start" as const] : []),
		...(!Number.isFinite(length) ? ["length" as const] : []),
	];
	if (nonFiniteFields.length > 0) {
		return { kind: "invalid", message: `${placementFieldNames(nonFiniteFields)} must be finite.`, invalidFields: nonFiniteFields };
	}
	const offGridFields: DefinitionChildDraftField[] = [
		...(!Number.isInteger(start * 4) ? ["start" as const] : []),
		...(!Number.isInteger(length * 4) ? ["length" as const] : []),
	];
	if (offGridFields.length > 0) {
		return { kind: "invalid", message: `${placementFieldNames(offGridFields)} must align to the 0.25 beat grid.`, invalidFields: offGridFields };
	}
	if (start < 0 || length <= 0 || start + length > parentLength) {
		const exceedsParent = start + length > parentLength;
		return {
			kind: "invalid",
			message: `Placement must stay within 0 to ${parentLength} beats.`,
			invalidFields: [
				...(start < 0 || exceedsParent ? ["start" as const] : []),
				...(length <= 0 || exceedsParent ? ["length" as const] : []),
			],
		};
	}
	const parsed = DefinitionChildSchema.safeParse({
		id: draft.childId ?? "server-allocated",
		definitionId: draft.definitionId,
		start,
		length,
	});
	return parsed.success
		? { kind: "valid", child: parsed.data }
		: { kind: "invalid", message: "Choose a valid child source and placement.", invalidFields: ["definitionId"] };
};

export const settleDefinitionChildDraft = (
	draft: DefinitionChildDraft,
	succeeded: boolean,
): DefinitionChildDraft | undefined => succeeded ? undefined : draft;

export const definitionPathSessionKey = (
	projectId: string,
	instance: Instance | undefined,
): string => `${projectId}:${instance?.id ?? "uninstantiated"}:${instance?.definitionId ?? "none"}`;

export const pianoRollSessionKey = (
	instance: Instance | undefined,
	sourceDefinitionId = instance?.definitionId,
): string => instance === undefined ? `source:${sourceDefinitionId ?? "empty"}` : `${instance.id}:${sourceDefinitionId ?? instance.definitionId}`;

export const sourceNoteLayers = (
	definitionId: string,
	ownedNotes: readonly Note[],
	compositionNotes: readonly CompositionNote[],
) => ({
	solid: ownedNotes,
	ghosts: compositionNotes.filter((note) => note.origin.definitionId !== definitionId),
});

export const provenanceLabel = (
	note: CompositionNote,
	definitionNames: ReadonlyMap<string, string>,
): string => {
	const origin = note.origin;
	const path = origin.definitionPath?.map((entry) =>
		`${definitionNames.get(entry.definitionId) ?? entry.definitionId} r${entry.definitionRevision}`,
	).join(" / ") ?? (definitionNames.get(origin.definitionId) ?? origin.definitionId);
	const leaf = origin.definitionPath?.at(-1);
	return [
		path,
		leaf?.placementId === undefined ? undefined : `placement ${leaf.placementId}`,
		leaf?.loopIndex === undefined ? undefined : `loop ${leaf.loopIndex}`,
		origin.instanceLoopIndex === undefined ? undefined : `instance loop ${origin.instanceLoopIndex}`,
		origin.modifierIds.length === 0 ? undefined : `modifiers ${origin.modifierIds.join(", ")}`,
		`note ${origin.noteId}`,
	].filter((part) => part !== undefined).join(" · ");
};

export const definitionChildCandidates = (
	definitions: readonly Definition[],
	parentDefinitionId: string,
): readonly Definition[] => {
	const reachesParent = (definitionId: string, visited: ReadonlySet<string>): boolean => {
		if (definitionId === parentDefinitionId) return true;
		if (visited.has(definitionId)) return false;
		const definition = definitions.find((item) => item.id === definitionId);
		const nextVisited = new Set(visited).add(definitionId);
		return (definition?.children ?? []).some((child) => reachesParent(child.definitionId, nextVisited));
	};
	return definitions.filter((definition) => !reachesParent(definition.id, new Set()));
};

export const addDefinitionChildCommand = (
	parentDefinitionId: string,
	child: DefinitionChild,
) => ({
	type: "addDefinitionChild",
	parentDefinitionId,
	definitionId: child.definitionId,
	start: child.start,
	length: child.length,
} as const);

export const setDefinitionChildCommand = (
	parentDefinitionId: string,
	child: DefinitionChild,
) => ({ type: "setDefinitionChild", parentDefinitionId, child } as const);

export const removeDefinitionChildCommand = (
	parentDefinitionId: string,
	childId: string,
) => ({ type: "removeDefinitionChild", parentDefinitionId, childId } as const);

export const editDefinitionNotesCommand = (
	definitionId: string,
	notes: readonly Note[],
) => ({ type: "editDefinitionNotes", definitionId, notes } as const);
