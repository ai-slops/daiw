import { describe, expect, it } from "vitest";
import { InstanceSchema } from "../domain/project";
import { pianoRollSessionKey } from "./piano-roll-session";

const instance = InstanceSchema.parse({
	id: "instance-a",
	name: "A",
	definitionId: "definition-a",
	trackId: "track-a",
	start: 0,
	length: 4,
	modifiers: [],
	overrides: [],
});

describe("piano-roll editor isolation", () => {
	it("uses a distinct session when the selected instance changes", () => {
		// Given two clips that share one source definition
		const sibling = InstanceSchema.parse({ ...instance, id: "instance-b" });

		// When their editor identities are derived
		const firstKey = pianoRollSessionKey(instance);
		const siblingKey = pianoRollSessionKey(sibling);

		// Then a draft-capable editor cannot reuse local state across clips
		expect(siblingKey).not.toBe(firstKey);
	});

	it("uses a distinct session when the source definition changes", () => {
		// Given one placement rebound to another definition
		const rebound = InstanceSchema.parse({
			...instance,
			definitionId: "definition-b",
		});

		// When their editor identities are derived
		const firstKey = pianoRollSessionKey(instance);
		const reboundKey = pianoRollSessionKey(rebound);

		// Then stale source-local state cannot cross the definition boundary
		expect(reboundKey).not.toBe(firstKey);
	});
});
