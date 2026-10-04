import { beatAtSeconds, secondsAtBeat } from "../../shared/timing.mjs";
import type { LoopRange } from "./types";

export { beatAtSeconds, secondsAtBeat };
export const beatAtTime = beatAtSeconds;

export function normalizeLoopBeat(beat: number, loop: LoopRange | null): number {
  if (loop === null || beat < loop.end) {
    return beat;
  }
  const length = loop.end - loop.start;
  return loop.start + ((beat - loop.start) % length);
}

export function validateLoop(loop: LoopRange, snapshotLength: number): LoopRange {
  if (!Number.isFinite(loop.start) || !Number.isFinite(loop.end) ||
      loop.start < 0 || loop.start >= loop.end || loop.end > snapshotLength) {
    throw new RangeError("loop must be inside the immutable snapshot");
  }
  return Object.freeze({ ...loop });
}
