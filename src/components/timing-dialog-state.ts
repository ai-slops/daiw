export type TimingSaveState =
	| Readonly<{ kind: "idle" }>
	| Readonly<{ kind: "pending"; baseRevision: string }>
	| Readonly<{ kind: "saved"; revision: string }>
	| Readonly<{ kind: "failed"; message: string }>;

export type TimingSaveUpdate = Readonly<{
	busy: boolean;
	error: string | undefined;
	revision: string;
}>;

export const nextTimingSession = (
	current: number,
	previousOpen: boolean,
	open: boolean,
): number => (!previousOpen && open ? current + 1 : current);

export const beginTimingSave = (baseRevision: string): TimingSaveState => ({
	kind: "pending",
	baseRevision,
});

export const settleTimingSave = (
	state: TimingSaveState,
	update: TimingSaveUpdate,
): TimingSaveState => {
	if (state.kind !== "pending" || update.busy) return state;
	if (update.error !== undefined)
		return { kind: "failed", message: update.error };
	if (update.revision !== state.baseRevision)
		return { kind: "saved", revision: update.revision };
	return state;
};
