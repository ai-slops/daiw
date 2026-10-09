import { afterEach, describe, expect, it, vi } from "vitest";
import {
	createEngine,
	FakeAudioContext,
	FakeBuffer,
	flush,
	snapshot,
} from "./engine.test-support";
import { CHUNK_FRAMES } from "./server-render";

const transport = vi.hoisted(() => ({ open: vi.fn(), chunk: vi.fn() }));
vi.mock("./server-render", async (load) => ({
	...(await load<typeof import("./server-render")>()),
	openServerRender: transport.open,
	fetchPcmChunk: transport.chunk,
}));

function descriptor(side = "a", frames = 8 * 48_000) {
	return {
		info: {
			format: "daiw-wav-pcm16-v1",
			revision: "revision-1",
			renderKey: `render-${side}`,
			sampleRate: 48_000,
			channels: 2,
			frames,
			byteLength: frames * 4 + 44,
			dataOffset: 44,
			bytesPerFrame: 4,
			sha256: "a".repeat(64),
			pcmRms: 0.5,
			pcmPeak: 0.8,
		},
		wavUrl: `/render-${side}.wav`,
		searchParams: new URLSearchParams(),
	};
}

function successfulChunks(): void {
	transport.chunk.mockImplementation(
		(_context, _render, range: { startFrame: number; endFrame: number }) =>
			Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
	);
}

describe("host-compatible playback regressions", () => {
	afterEach(() => {
		transport.open.mockReset();
		transport.chunk.mockReset();
		vi.unstubAllGlobals();
	});

	it("schedules the first source with lead time and clamps position before its fixed anchor", async () => {
		// Given
		const context = new FakeAudioContext();
		context.currentTime = 3;
		context.advanceOnCreateSource = 0.001;
		transport.open.mockResolvedValue(descriptor());
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");

		// When
		await engine.play();

		// Then
		const firstWhen = context.sources[0]?.start.mock.calls[0]?.[0];
		expect(firstWhen).toBeGreaterThanOrEqual(3.05);
		expect(engine.getPositionBeats()).toBe(0);
	});

	it("stops at a partial final chunk without requesting an empty EOF range or scheduling a zero-duration node", async () => {
		// Given
		const frames = CHUNK_FRAMES + 1;
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor("a", frames));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot(frames / 48_000));
		await engine.prepare("project-1");

		// When
		await engine.play();
		await flush();
		context.sources[0]?.end();
		context.sources[1]?.end();

		// Then
		const ranges: Array<{ startFrame: number; endFrame: number }> =
			transport.chunk.mock.calls.map((call) => call[2]);
		expect(ranges.every((range) => range.startFrame < range.endFrame)).toBe(
			true,
		);
		expect(context.sources).toHaveLength(2);
		expect(engine.getState().status).toBe("stopped");
	});

	it("plays a prefix before entering a one-frame native loop at the exact sequence boundary", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		engine.setLoop({ start: 1, end: 1 + 1 / 48_000 });
		engine.seek(0);

		// When
		await engine.play();

		// Then
		expect(context.sources[0]?.loop).toBe(false);
		expect(context.sources[0]?.start).toHaveBeenCalledWith(0.05, 0, 0.5);
		expect(context.sources[1]?.start).toHaveBeenCalledWith(0.55, 0, 0.5);
		const nativeLoop = context.sources.at(-1);
		expect(nativeLoop?.loop).toBe(true);
		expect(nativeLoop?.start).toHaveBeenCalledWith(1.05, 0);
	});

	it("keeps composite loop buffers inside the eight-buffer and byte bounds", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		engine.setLoop({ start: 0.9, end: 1.1 });

		// When
		await engine.play();

		// Then
		const metrics = engine.getStreamingMetrics().branches.a;
		expect(metrics.uniqueBuffers).toBeLessThanOrEqual(8);
		expect(metrics.retainedBytes).toBeLessThanOrEqual(8 * CHUNK_FRAMES * 2 * 4);
	});

	it("aborts and cleans both temporary A/B branches when either preparation fails", async () => {
		// Given
		const context = new FakeAudioContext();
		const signals: AbortSignal[] = [];
		transport.open.mockImplementation((_request, signal: AbortSignal) => {
			signals.push(signal);
			return signals.length === 1
				? Promise.resolve(descriptor("a"))
				: Promise.reject(new TypeError("B failed"));
		});
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());

		// When
		const preparation = engine.prepareAB("project-1", "proposal-1");

		// Then
		await expect(preparation).rejects.toThrow("B failed");
		expect(signals.every((signal) => signal.aborted)).toBe(true);
		expect(engine.getStreamingMetrics().branches.a.uniqueBuffers).toBe(0);
		expect(engine.getStreamingMetrics().branches.b.uniqueBuffers).toBe(0);
	});

	it("atomically reschedules a playing A/B replacement at the current logical frame", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor("old"))
			.mockResolvedValueOnce(descriptor("a"))
			.mockResolvedValueOnce(descriptor("b"));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();
		context.currentTime = 0.3;
		const expectedOffset = engine.getPositionBeats();

		// When
		await engine.prepareAB("project-1", "proposal-1");

		// Then
		expect(engine.getState().status).toBe("playing");
		expect(engine.getState().audition).toBe("a");
		const starts = context.sources.flatMap((source) => source.start.mock.calls);
		expect(
			starts.some(
				([when, offset]) =>
					when === 0.35 &&
					offset !== undefined &&
					Math.abs(offset - (expectedOffset + 0.05)) < 0.0001,
			),
		).toBe(true);
	});

	it("treats a snapshot abort during play priming as expected control flow", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor());
		let requests = 0;
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: {
					startFrame: number;
					endFrame: number;
					signal: AbortSignal;
				},
			) => {
				requests += 1;
				if (requests <= 2)
					return Promise.resolve(
						new FakeBuffer(range.endFrame - range.startFrame),
					);
				return new Promise((_resolve, reject) => {
					range.signal.addEventListener(
						"abort",
						() => reject(new DOMException("aborted", "AbortError")),
						{ once: true },
					);
				});
			},
		);
		const engine = createEngine(context);
		const faults: Error[] = [];
		engine.subscribeFault((fault) => faults.push(fault));
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		engine.seek(2);

		// When
		const playback = engine.play();
		await Promise.resolve();
		engine.setSnapshot(snapshot(8, "revision-2"));

		// Then
		await expect(playback).resolves.toBeUndefined();
		expect(faults).toEqual([]);
	});

	it("removes an obsolete B branch when preparing a single replacement", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor("a"))
			.mockResolvedValueOnce(descriptor("b"))
			.mockResolvedValueOnce(descriptor("next"));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepareAB("project-1", "proposal-1");

		// When
		await engine.prepare("project-1");

		// Then
		expect(engine.getStreamingMetrics().branches.b.uniqueBuffers).toBe(0);
		expect(engine.getState().audition).toBe("a");
	});
});
