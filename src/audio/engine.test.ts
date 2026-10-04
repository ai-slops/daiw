import { afterEach, describe, expect, it, vi } from "vitest";
import { DaiwAudioEngine } from "./engine";

const serverRender = vi.hoisted(() => ({ fetch: vi.fn() }));
vi.mock("./server-render", async (load) => ({
  ...(await load<typeof import("./server-render")>()),
  fetchServerRender: serverRender.fetch,
}));

class FakeAnalyser {
  public fftSize = 32;
  public smoothingTimeConstant = 0;

  public connect(): void {}

  public getFloatTimeDomainData(): void {}
}

class FakeSource {
  public buffer: AudioBuffer | null = null;
  public loop = false;
  public loopStart = 0;
  public loopEnd = 0;
  public onended: (() => void) | null = null;
  public readonly start = vi.fn();
  public connect(node: FakeGain): FakeGain { return node; }
  public disconnect(): void {}
  public stop(): void {}
}

class FakeGain {
  public readonly gain = { value: 1 };
  public connect(node: FakeAnalyser): FakeAnalyser { return node; }
  public disconnect(): void {}
}

class FakeAudioContext {
  public static latest: FakeAudioContext | null = null;
  public currentTime = 0;
  public readonly destination = {};
  public readonly sources: FakeSource[] = [];

  public constructor() { FakeAudioContext.latest = this; }

  public createAnalyser(): FakeAnalyser {
    return new FakeAnalyser();
  }

  public createBufferSource(): FakeSource {
    const source = new FakeSource();
    this.sources.push(source);
    return source;
  }

  public createGain(): FakeGain { return new FakeGain(); }
  public resume(): Promise<void> { return Promise.resolve(); }
}

function snapshot(revision: string, length: number) {
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
  afterEach(() => {
    vi.unstubAllGlobals();
    serverRender.fetch.mockReset();
  });

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

  it("uses mapped absolute seconds for seek, elapsed position, and loop offsets", async () => {
    // Given
    vi.stubGlobal("AudioContext", FakeAudioContext);
    serverRender.fetch.mockResolvedValue({ duration: 30 });
    const engine = new DaiwAudioEngine();
    const context = FakeAudioContext.latest;
    if (context === null) throw new TypeError("fake audio context was not constructed");
    engine.setSnapshot({
      ...snapshot("revision-1", 12),
      tempoMap: { points: [{ at: 0, bpm: 120, curve: "step" }, { at: 4, bpm: 60, curve: "step" }] },
    });
    await engine.prepare("project-1");
    engine.seek(5);
    engine.setLoop({ start: 4, end: 8 });

    // When
    await engine.play();
    context.currentTime = 4;

    // Then
    const source = context.sources.at(-1);
    expect(source?.start).toHaveBeenCalledWith(0, 3);
    expect(source?.loopStart).toBe(2);
    expect(source?.loopEnd).toBe(6);
    expect(engine.getPositionBeats()).toBe(5);
  });

  it("normalizes a seek outside the active loop before scheduling audio", async () => {
    // Given
    vi.stubGlobal("AudioContext", FakeAudioContext);
    serverRender.fetch.mockResolvedValue({ duration: 30 });
    const engine = new DaiwAudioEngine();
    const context = FakeAudioContext.latest;
    if (context === null) throw new TypeError("fake audio context was not constructed");
    engine.setSnapshot(snapshot("revision-1", 12));
    await engine.prepare("project-1");
    engine.setLoop({ start: 4, end: 8 });
    engine.seek(10);

    // When
    await engine.play();

    // Then
    expect(context.sources.at(-1)?.start).toHaveBeenCalledWith(0, 3);
    expect(engine.getPositionBeats()).toBe(6);
  });

  it("rejects nonfinite loop endpoints", () => {
    // Given
    vi.stubGlobal("AudioContext", FakeAudioContext);
    const engine = new DaiwAudioEngine();
    engine.setSnapshot(snapshot("revision-1", 12));

    // When
    const setLoop = () => engine.setLoop({ start: Number.NaN, end: 8 });

    // Then
    expect(setLoop).toThrow(RangeError);
  });
});
