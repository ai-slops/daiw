import type { LoopRange } from "./types";

export function secondsAtBeat(beat: number, tempo: number): number {
  return (beat * 60) / tempo;
}

export function beatAtTime(seconds: number, tempo: number): number {
  return (seconds * tempo) / 60;
}

export function normalizeLoopBeat(beat: number, loop: LoopRange | null): number {
  if (loop === null || beat < loop.end) {
    return beat;
  }
  const length = loop.end - loop.start;
  return loop.start + ((beat - loop.start) % length);
}

export function validateLoop(loop: LoopRange, snapshotLength: number): LoopRange {
  if (loop.start < 0 || loop.start >= loop.end || loop.end > snapshotLength) {
    throw new RangeError("loop must be inside the immutable snapshot");
  }
  return Object.freeze({ ...loop });
}
