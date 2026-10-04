import { describe, expect, it } from "vitest";
import {
	beginTimingSave,
	nextTimingSession,
	settleTimingSave,
} from "./timing-dialog-state";

describe("timing dialog state", () => {
	it("starts a fresh editor session when a closed dialog reopens", () => {
		// Given
		const previousSession = 4;

		// When
		const closed = nextTimingSession(previousSession, true, false);
		const reopened = nextTimingSession(closed, false, true);

		// Then
		expect(closed).toBe(previousSession);
		expect(reopened).toBe(previousSession + 1);
	});

	it("does not report saved before the canonical revision changes", () => {
		// Given
		const pending = beginTimingSave("rev-1");

		// When
		const unchanged = settleTimingSave(pending, {
			busy: false,
			error: undefined,
			revision: "rev-1",
		});

		// Then
		expect(unchanged).toEqual({ kind: "pending", baseRevision: "rev-1" });
	});

	it("reports saved only after the canonical revision advances", () => {
		// Given
		const pending = beginTimingSave("rev-1");

		// When
		const saved = settleTimingSave(pending, {
			busy: false,
			error: undefined,
			revision: "rev-2",
		});

		// Then
		expect(saved).toEqual({ kind: "saved", revision: "rev-2" });
	});

	it("keeps a stale-revision failure visible after busy clears", () => {
		// Given
		const pending = beginTimingSave("rev-1");

		// When
		const failed = settleTimingSave(pending, {
			busy: false,
			error: "StaleRevision: expected rev-1",
			revision: "rev-1",
		});

		// Then
		expect(failed).toEqual({
			kind: "failed",
			message: "StaleRevision: expected rev-1",
		});
	});
});
