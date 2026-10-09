export type LoopChunk = Readonly<{
	start: number;
	end: number;
	buffer: AudioBuffer;
}>;

type FrameRange = Readonly<{
	start: number;
	end: number;
}>;

export function assembleLoopBuffer(
	output: AudioBuffer,
	range: FrameRange,
	chunkAt: (frame: number) => LoopChunk | null,
): void {
	let destination = 0;
	let cursor = range.start;
	while (cursor < range.end) {
		const chunk = chunkAt(cursor);
		if (chunk === null) throw new RangeError("loop PCM is unavailable");
		const count = Math.min(range.end, chunk.end) - cursor;
		for (let channel = 0; channel < 2; channel += 1) {
			output.copyToChannel(
				chunk.buffer
					.getChannelData(channel)
					.subarray(cursor - chunk.start, cursor - chunk.start + count),
				channel,
				destination,
			);
		}
		cursor += count;
		destination += count;
	}
}
