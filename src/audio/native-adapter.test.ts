import { describe, expect, it } from "vitest";
import {
	nativeAudioEvent,
	nativeAudioState,
	selectNativeDevice,
	shouldAcceptNativeCompletion,
} from "./native-adapter";
import type { Snapshot } from "./types";

const snapshot = {
	revision: "revision-1",
	tempo: 120,
	meter: [4, 4],
	length: 8,
	tracks: [],
	notes: [],
	regions: [],
} satisfies Snapshot;

describe("native audio adapter", () => {
	it("maps worker frames through the snapshot tempo map", () => {
		// Given
		const mapped = {
			...snapshot,
			tempoMap: {
				points: [
					{ at: 0, bpm: 120, curve: "step" as const },
					{ at: 4, bpm: 60, curve: "step" as const },
				],
			},
		};
		// When
		const state = nativeAudioState(mapped, {
			status: "playing",
			positionFrame: 192_000,
			frames: 240_000,
			loop: null,
			stats: { underrunCallbacks: 0, staleBlocks: 0, deviceErrors: 0 },
		});
		// Then
		expect(state.positionBeat).toBe(6);
		expect(state.status).toBe("playing");
	});

	it("preserves a current supported device and otherwise chooses the default", () => {
		// Given
		const devices = [
			{
				token: "default-token",
				name: "Default",
				isDefault: true,
				supported: true,
			},
			{
				token: "other-token",
				name: "Other",
				isDefault: false,
				supported: true,
			},
		];
		// When / Then
		expect(selectNativeDevice("other-token", devices)).toBe("other-token");
		expect(selectNativeDevice("stale-token", devices)).toBe("default-token");
		expect(
			selectNativeDevice(
				undefined,
				devices.map((device) => ({ ...device, supported: false })),
			),
		).toBeUndefined();
	});

	it("rejects stale completion after generation, project, revision, or backend changes", () => {
		// Given
		const expected = {
			backend: "native" as const,
			sessionToken: "session-token-11111111",
			projectId: "project-1",
			revision: "revision-1",
		};
		// When / Then
		expect(shouldAcceptNativeCompletion(expected, expected)).toBe(true);
		expect(
			shouldAcceptNativeCompletion(
				{ ...expected, backend: "browser" },
				expected,
			),
		).toBe(false);
		expect(
			shouldAcceptNativeCompletion(
				{ ...expected, sessionToken: "session-token-22222222" },
				expected,
			),
		).toBe(false);
		expect(
			shouldAcceptNativeCompletion(
				{ ...expected, revision: "revision-2" },
				expected,
			),
		).toBe(false);
	});

	it("maps a matching terminal worker event to a visible fault", () => {
		// Given
		const current = {
			backend: "native" as const,
			sessionToken: "session-token-11111111",
			projectId: "project-1",
			revision: "revision-1",
		};
		// When
		const result = nativeAudioEvent(snapshot, current, {
			sessionToken: current.sessionToken,
			projectId: current.projectId,
			expectedRevision: current.revision,
			fault: { code: "WorkerClosed", message: "output stopped" },
		});
		// Then
		expect(result).toEqual({
			state: {
				status: "faulted",
				positionBeat: 0,
				loop: null,
				revision: "revision-1",
				audition: null,
				loudnessMatching: false,
			},
			fault: "output stopped",
		});
	});
});
