import type { Instance } from "../domain/project";

export const pianoRollSessionKey = (instance: Instance | undefined): string =>
	instance === undefined ? "empty" : `${instance.id}:${instance.definitionId}`;
