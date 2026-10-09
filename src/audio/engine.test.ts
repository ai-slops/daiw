import { afterEach, describe, expect, it, vi } from "vitest";
import {
	createEngine,
	FakeAudioContext,
	FakeBuffer,
	flush,
	snapshot,
} from "./engine.test-support";
import { UnderrunError } from "./errors";
import { CHUNK_FRAMES } from "./server-render";

const transport = vi.hoisted(() => ({ open: vi.fn(), chunk: vi.fn() }));
vi.mock("./server-render", async (load) => ({
	...(await load<typeof import("./server-render")>()),
	openServerRender: transport.open,
	fetchPcmChunk: transport.chunk,
}));

const INFO = {
	format: "daiw-wav-pcm16-v1",
	revision: "revision-1",
	renderKey: "render-a",
	sampleRate: 48_000,
	channels: 2,
	frames: 8 * 48_000,
	byteLength: 8 * 48_000 * 4 + 44,
	dataOffset: 44,
	bytesPerFrame: 4,
	sha256: "a".repeat(64),
	pcmRms: 0.5,
	pcmPeak: 0.8,
} as const;

function descriptor(side = "a", frames = INFO.frames) {
	return {
		info: {
			...INFO,
			renderKey: `render-${side}`,
			frames,
			byteLength: frames * 4 + 44,
		},
		wavUrl: `/render-${side}.wav`,
		searchParams: new URLSearchParams(),
	};
}

describe("bounded PCM playback engine", () => {
	afterEach(() => {
		transport.open.mockReset();
		transport.chunk.mockReset();
		vi.unstubAllGlobals();
	});

	it("primes one second then schedules successors at the exact predecessor end", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		transport.chunk.mockImplementation(
			(_context, _render, range: { startFrame: number; endFrame: number }) =>
				Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");

		// When
		await engine.play();
		await flush();

		// Then
		expect(
			transport.chunk.mock.calls.slice(0, 2).map((call) => call[2]),
		).toEqual([
			expect.objectContaining({ startFrame: 0, endFrame: CHUNK_FRAMES }),
			expect.objectContaining({
				startFrame: CHUNK_FRAMES,
				endFrame: 2 * CHUNK_FRAMES,
			}),
		]);
		expect(context.sources[0]?.start).toHaveBeenCalledWith(0.05, 0, 0.5);
		expect(context.sources[1]?.start).toHaveBeenCalledWith(0.55, 0, 0.5);
		expect(
			engine.getStreamingMetrics().branches.a.inflightRanges,
		).toBeLessThanOrEqual(1);
	});

	it("maps tempo seconds to rounded 48k frames when seeking", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor("a", 480_000));
		transport.chunk.mockImplementation(
			(_context, _render, range: { startFrame: number; endFrame: number }) =>
				Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot({
			...snapshot(12),
			tempo: 120,
			tempoMap: {
				points: [
					{ at: 0, bpm: 120, curve: "step" },
					{ at: 4, bpm: 60, curve: "step" },
				],
			},
		});
		await engine.prepare("project-1");

		// When
		engine.seek(5);
		await engine.play();

		// Then
		expect(context.sources[0]?.start).toHaveBeenCalledWith(0.05, 0, 0.5);
		expect(transport.chunk).toHaveBeenCalledWith(
			expect.anything(),
			expect.anything(),
			expect.objectContaining({
				startFrame: 144_000,
				endFrame: 168_000,
				signal: expect.any(AbortSignal),
			}),
		);
	});

	it("uses one native looping source for a one-frame end-exclusive loop", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		transport.chunk.mockImplementation(
			(_context, _render, range: { startFrame: number; endFrame: number }) =>
				Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		engine.setLoop({ start: 1, end: 1 + 1 / 48_000 });
		engine.seek(1);

		// When
		await engine.play();

		// Then
		const source = context.sources.at(-1);
		expect(source?.buffer?.length).toBe(1);
		expect(source?.loop).toBe(true);
		expect(source?.loopEnd).toBe(1 / 48_000);
	});

	it("stops naturally at the exclusive render boundary without an underrun", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor("a", CHUNK_FRAMES));
		transport.chunk.mockResolvedValue(new FakeBuffer(CHUNK_FRAMES));
		const engine = createEngine(context);
		const faults: Error[] = [];
		engine.subscribeFault((fault) => faults.push(fault));
		engine.setSnapshot(snapshot(0.5));
		await engine.prepare("project-1");
		await engine.play();

		// When
		context.sources.at(-1)?.end();

		// Then
		expect(engine.getState().status).toBe("stopped");
		expect(engine.getState().positionBeat).toBe(0.5);
		expect(faults).toEqual([]);
	});

	it("pauses at the first missing frame and requires explicit play after a network stall", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		transport.chunk
			.mockResolvedValueOnce(new FakeBuffer(CHUNK_FRAMES))
			.mockResolvedValueOnce(new FakeBuffer(CHUNK_FRAMES))
			.mockImplementation(() => new Promise(() => undefined));
		const engine = createEngine(context);
		const faults: Error[] = [];
		engine.subscribeFault((fault) => faults.push(fault));
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();

		// When
		context.sources[0]?.end();
		context.sources[1]?.end();

		// Then
		expect(engine.getState().status).toBe("paused");
		expect(engine.getState().positionBeat).toBe(1);
		expect(faults[0]).toBeInstanceOf(UnderrunError);
	});

	it("atomically primes A/B, switches synchronously, and aborts stale generations", async () => {
		// Given
		const context = new FakeAudioContext();
		const signals: AbortSignal[] = [];
		transport.open.mockImplementation((_request, signal: AbortSignal) => {
			signals.push(signal);
			return Promise.resolve(descriptor(signals.length === 1 ? "a" : "b"));
		});
		transport.chunk.mockImplementation(
			(_context, _render, range: { startFrame: number; endFrame: number }) =>
				Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepareAB("project-1", "proposal-1");

		// When
		engine.selectAB("b");
		engine.setSnapshot(snapshot(8, "revision-2"));

		// Then
		expect(signals.every((signal) => signal.aborted)).toBe(true);
		expect(engine.getState().audition).toBeNull();
	});

	it("never retains more than eight unique buffers per branch for a 180-second render", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor("a", 180 * 48_000));
		transport.chunk.mockImplementation(
			(_context, _render, range: { startFrame: number; endFrame: number }) =>
				Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot(180));
		await engine.prepare("project-1");
		await engine.play();

		// When
		for (let index = 0; index < 20; index += 1) {
			await flush();
			context.sources[index]?.end();
		}

		// Then
		expect(
			engine.getStreamingMetrics().branches.a.uniqueBuffers,
		).toBeLessThanOrEqual(8);
		expect(engine.getStreamingMetrics().maxUniqueBuffersPerBranch).toBe(8);
	});

	it("disposes owned context, graph, sources, and in-flight requests", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		transport.chunk.mockResolvedValue(new FakeBuffer(CHUNK_FRAMES));
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");

		// When
		await engine.dispose();

		// Then
		expect(context.closed).toBe(1);
	});
});
