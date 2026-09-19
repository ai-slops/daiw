import { afterEach, describe, expect, it, vi } from "vitest";
import { DaiwAudioEngine } from "./engine";

class FakeAnalyser {
  public fftSize = 32;
  public smoothingTimeConstant = 0;

  public connect(): void {}
}

class FakeAudioContext {
  public readonly currentTime = 0;
  public readonly destination = {};

  public createAnalyser(): FakeAnalyser {
    return new FakeAnalyser();
  }
}

function snapshot(revision: string, length: number): unknown {
  return {
    revision,
    tempo: 120,
    length,
    tracks: [],
    notes: [],
    regions: [],
  };
}

describe("snapshot transport boundary", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("clears the prior snapshot loop when replacing state", () => {
    // Given
    vi.stubGlobal("AudioContext", FakeAudioContext);
    const engine = new DaiwAudioEngine();
    engine.setSnapshot(snapshot("revision-1", 8));
    engine.setLoop({ start: 4, end: 8 });

    // When
    engine.setSnapshot(snapshot("revision-2", 1));

    // Then
    expect(engine.getState().loop).toBeNull();
  });
});
