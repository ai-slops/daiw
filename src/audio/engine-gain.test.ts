import { afterEach, describe, expect, it, vi } from "vitest";
import {
	createEngine,
	FakeAudioContext,
	FakeBuffer,
	snapshot,
} from "./engine.test-support";

const transport = vi.hoisted(() => ({ open: vi.fn(), chunk: vi.fn() }));
vi.mock("./server-render", async (load) => ({
	...(await load<typeof import("./server-render")>()),
	openServerRender: transport.open,
	fetchPcmChunk: transport.chunk,
}));

const descriptor = (key: string, rms: number) => ({
	info: {
		format: "daiw-wav-pcm16-v1",
		revision: "revision-1",
		renderKey: key,
		sampleRate: 48_000,
		channels: 2,
		frames: 8 * 48_000,
		byteLength: 8 * 48_000 * 4 + 44,
		dataOffset: 44,
		bytesPerFrame: 4,
		sha256: "a".repeat(64),
		pcmRms: rms,
		pcmPeak: 0.8,
	},
	wavUrl: `/${key}.wav`,
	searchParams: new URLSearchParams(),
});

describe("A/B gain transitions", () => {
	afterEach(() => {
		transport.open.mockReset();
		transport.chunk.mockReset();
		vi.unstubAllGlobals();
	});

	it("keeps the old gain until the new branch starts at the audio boundary", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor("a", 0.5))
			.mockResolvedValueOnce(descriptor("b", 0.25));
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepareAB("project-1", "proposal-1");
		engine.setLoudnessMatching(true);
		await engine.play();
		context.currentTime = 0.3;

		// When
		engine.selectAB("b");

		// Then
		expect(context.gain.gain.value).toBe(1);
		expect(context.gain.gain.automation.at(-1)).toEqual({
			value: 2,
			time: 0.35,
		});
		expect(
			context.sources
				.flatMap((source) => source.start.mock.calls)
				.some(([when]) => when === 0.35),
		).toBe(true);
		context.gain.gain.advance(0.349);
		expect(context.gain.gain.value).toBe(1);
		context.gain.gain.advance(0.35);
		expect(context.gain.gain.value).toBe(2);
	});

	it("cancels a stale B gain event when a rapid switch schedules A", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor("a", 0.5))
			.mockResolvedValueOnce(descriptor("b", 0.25));
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepareAB("project-1", "proposal-1");
		engine.setLoudnessMatching(true);
		await engine.play();
		context.currentTime = 0.3;
		engine.selectAB("b");

		// When
		context.currentTime = 0.31;
		engine.selectAB("a");

		// Then
		expect(
			context.gain.gain.automation.filter((event) => event.time >= 0.31),
		).toEqual([{ value: 1, time: 0.36 }]);
	});

	it("cancels pending gain automation on stop", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor("a", 0.5))
			.mockResolvedValueOnce(descriptor("b", 0.25));
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepareAB("project-1", "proposal-1");
		engine.setLoudnessMatching(true);
		await engine.play();
		context.currentTime = 0.3;
		engine.selectAB("b");

		// When
		engine.stop();

		// Then
		expect(
			context.gain.gain.automation.filter((event) => event.time >= 0.3),
		).toEqual([]);
	});
});
