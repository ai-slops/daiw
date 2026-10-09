import { loopFrames } from "./playback-frames";
import { validateLoop } from "./transport";
import type { LoopRange, Snapshot } from "./types";

export function validateRenderedLoop(
	snapshot: Snapshot,
	loop: LoopRange | null,
): LoopRange | null {
	if (loop === null) return null;
	const validated = validateLoop(loop, snapshot.length);
	const frames = loopFrames(snapshot, validated);
	if (frames === null || frames.end <= frames.start)
		throw new RangeError("loop must span at least one rendered frame");
	return validated;
}
