import { describe, expect, it } from "vitest";
import { renderRevisionMatches, snapshotRevisionMatches } from "./server-render";

describe("server render provenance", () => {
  it("rejects a missing or mismatched response revision", () => {
    // Given
    const expected = "revision-2";

    // When
    const missing = renderRevisionMatches(expected, null);
    const stale = renderRevisionMatches(expected, "revision-1");

    // Then
    expect(missing).toBe(false);
    expect(stale).toBe(false);
  });

  it("rejects decoded buffers when the active snapshot changed while awaiting", () => {
    // Given
    const captured = "revision-1";

    // When
    const matches = snapshotRevisionMatches(captured, "revision-2");

    // Then
    expect(matches).toBe(false);
  });

  it("accepts only the exact captured revision end to end", () => {
    // Given
    const captured = "revision-3";

    // When
    const headerMatches = renderRevisionMatches(captured, "revision-3");
    const snapshotMatches = snapshotRevisionMatches(captured, "revision-3");

    // Then
    expect(headerMatches && snapshotMatches).toBe(true);
  });
});
