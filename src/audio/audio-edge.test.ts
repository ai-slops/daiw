import { afterEach, describe, expect, it, vi } from "vitest";
import {
	createEngine,
	FakeAudioContext,
	FakeBuffer,
	snapshot,
} from "./engine.test-support";
import type { RenderDescriptor } from "./server-render";
import { CHUNK_FRAMES } from "./server-render";
import { StreamBranch } from "./stream-branch";

const transport = vi.hoisted(() => ({ open: vi.fn(), chunk: vi.fn() }));
vi.mock("./server-render", async (load) => ({
	...(await load<typeof import("./server-render")>()),
	openServerRender: transport.open,
	fetchPcmChunk: transport.chunk,
}));

const descriptor = (
	frames = 8 * 48_000,
	rms = 0.5,
	key = "a",
): RenderDescriptor => ({
	info: {
		format: "daiw-wav-pcm16-v1",
		revision: "revision-1",
		renderKey: key,
		sampleRate: 48_000,
		channels: 2,
		frames,
		byteLength: frames * 4 + 44,
		dataOffset: 44,
		bytesPerFrame: 4,
		sha256: "a".repeat(64),
		pcmRms: rms,
		pcmPeak: 0.8,
	},
	wavUrl: `/${key}.wav`,
	searchParams: new URLSearchParams(),
});

const successfulChunks = () =>
	transport.chunk.mockImplementation(
		(
			_context,
			_render,
			range: { readonly startFrame: number; readonly endFrame: number },
		) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
	);

describe("audio terminal and bounded-buffer semantics", () => {
	afterEach(() => {
		transport.open.mockReset();
		transport.chunk.mockReset();
		vi.unstubAllGlobals();
	});

	it("stays stopped without creating a source when play is requested again at natural EOF", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor(CHUNK_FRAMES));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot(0.5));
		await engine.prepare("project-1");
		await engine.play();
		context.sources[0]?.end();
		const sourceCount = context.sources.length;

		// When
		await engine.play();

		// Then
		expect(engine.getState().status).toBe("stopped");
		expect(context.sources).toHaveLength(sourceCount);
	});

	it("keeps a zero-frame snapshot stopped without creating zero-duration sources", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open.mockResolvedValue(descriptor(0));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot(0));
		await engine.prepare("project-1");

		// When
		await engine.play();

		// Then
		expect(engine.getState().status).toBe("stopped");
		expect(context.sources).toEqual([]);
	});

	it("rejects a positive beat loop that rounds to zero frames", () => {
		// Given
		const engine = createEngine(new FakeAudioContext());
		engine.setSnapshot(snapshot());

		// When
		const setLoop = () => engine.setLoop({ start: 1, end: 1 + 1 / 200_000 });

		// Then
		expect(setLoop).toThrow(RangeError);
	});

	it("pins loop input chunks while reserving a composite buffer at full cache capacity", async () => {
		// Given
		const context = new FakeAudioContext();
		successfulChunks();
		const branch = new StreamBranch(context, descriptor(12 * CHUNK_FRAMES));
		const signal = new AbortController().signal;
		for (let index = 0; index < 8; index += 1)
			await branch.ensure(index * CHUNK_FRAMES, signal);

		// When
		const loop = await branch.loopBuffer(
			CHUNK_FRAMES - 1,
			CHUNK_FRAMES + 1,
			signal,
		);

		// Then
		expect(loop.length).toBe(2);
		expect(branch.metrics().uniqueBuffers).toBeLessThanOrEqual(8);
	});

	it("uses unity loudness gain when either A or B is silent", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor(undefined, 0, "a"))
			.mockResolvedValueOnce(descriptor(undefined, 0.5, "b"));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepareAB("project-1", "proposal-1");
		engine.selectAB("b");

		// When
		engine.setLoudnessMatching(true);

		// Then
		expect(context.gain.gain.value).toBe(1);
	});

	it("reports aggregate ownership across installed A/B buffers with an explicit hard limit", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor(undefined, 0.5, "a"))
			.mockResolvedValueOnce(descriptor(undefined, 0.5, "b"));
		successfulChunks();
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());

		// When
		await engine.prepareAB("project-1", "proposal-1");
		const metrics = engine.getStreamingMetrics();

		// Then
		expect(metrics.overall.uniqueBuffers).toBe(4);
		expect(metrics.maxUniqueBuffersOverall).toBe(16);
	});
});
