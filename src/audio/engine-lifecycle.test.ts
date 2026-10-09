import { afterEach, describe, expect, it, vi } from "vitest";
import {
	createEngine,
	FakeAudioContext,
	FakeBuffer,
	flush,
	snapshot,
} from "./engine.test-support";

const transport = vi.hoisted(() => ({ open: vi.fn(), chunk: vi.fn() }));
vi.mock("./server-render", async (load) => ({
	...(await load<typeof import("./server-render")>()),
	openServerRender: transport.open,
	fetchPcmChunk: transport.chunk,
}));

const descriptor = (renderKey: string) => ({
	info: {
		format: "daiw-wav-pcm16-v1",
		revision: "revision-1",
		renderKey,
		sampleRate: 48_000,
		channels: 2,
		frames: 8 * 48_000,
		byteLength: 8 * 48_000 * 4 + 44,
		dataOffset: 44,
		bytesPerFrame: 4,
		sha256: "a".repeat(64),
		pcmRms: 0.5,
		pcmPeak: 0.8,
	},
	wavUrl: `/${renderKey}.wav`,
	searchParams: new URLSearchParams(),
});

function deferredDescriptor() {
	let resolve: ((value: ReturnType<typeof descriptor>) => void) | undefined;
	const promise = new Promise<ReturnType<typeof descriptor>>((accept) => {
		resolve = accept;
	});
	return {
		promise,
		release: (value: ReturnType<typeof descriptor>) => resolve?.(value),
	};
}

describe("audio preparation lifecycle", () => {
	afterEach(() => {
		transport.open.mockReset();
		transport.chunk.mockReset();
		vi.unstubAllGlobals();
	});

	it("keeps the live refill signal active while a cold replacement is preparing", async () => {
		// Given
		const context = new FakeAudioContext();
		let resolveReplacement:
			| ((value: ReturnType<typeof descriptor>) => void)
			| undefined;
		transport.open
			.mockResolvedValueOnce(descriptor("old"))
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						resolveReplacement = resolve;
					}),
			);
		const refillSignals: AbortSignal[] = [];
		transport.chunk.mockImplementation(
			(
				_context,
				render: ReturnType<typeof descriptor>,
				range: {
					readonly startFrame: number;
					readonly endFrame: number;
					readonly signal: AbortSignal;
				},
			) => {
				if (render.info.renderKey === "old") refillSignals.push(range.signal);
				return Promise.resolve(
					new FakeBuffer(range.endFrame - range.startFrame),
				);
			},
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();
		await flush();
		const liveSignal = refillSignals.at(-1);

		// When
		const preparation = engine.prepare("project-1");
		await Promise.resolve();

		// Then
		expect(liveSignal?.aborted).toBe(false);
		resolveReplacement?.(descriptor("replacement"));
		await preparation;
		expect(liveSignal?.aborted).toBe(true);
	});

	it("keeps old sources connected until the replacement boundary and advances the replacement frame", async () => {
		// Given
		const context = new FakeAudioContext();
		transport.open
			.mockResolvedValueOnce(descriptor("old"))
			.mockResolvedValueOnce(descriptor("replacement"));
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();
		await flush();
		context.currentTime = 0.3;
		const oldSource = context.sources[0];

		// When
		await engine.prepare("project-1");

		// Then
		expect(oldSource?.stop).toHaveBeenCalledWith(0.35);
		expect(oldSource?.disconnect).not.toHaveBeenCalled();
		expect(
			context.sources.flatMap((source) => source.start.mock.calls),
		).toContainEqual([0.35, 0.3, 0.2]);
		oldSource?.end();
		expect(oldSource?.disconnect).toHaveBeenCalledOnce();
	});

	it("keeps an explicit stop at frame zero when playing A/B preparation completes late", async () => {
		// Given
		const context = new FakeAudioContext();
		const reference = deferredDescriptor();
		const candidate = deferredDescriptor();
		transport.open
			.mockResolvedValueOnce(descriptor("old"))
			.mockReturnValueOnce(reference.promise)
			.mockReturnValueOnce(candidate.promise);
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();
		const ownedBefore = engine.getStreamingMetrics().overall.uniqueBuffers;
		const preparation = engine.prepareAB("project-1", "proposal-1");
		await Promise.resolve();

		// When
		engine.stop();
		const sourceCount = context.sources.length;
		reference.release(descriptor("a"));
		candidate.release(descriptor("b"));
		await preparation;

		// Then
		expect(engine.getState().status).toBe("stopped");
		expect(engine.getState().positionBeat).toBe(0);
		expect(context.sources).toHaveLength(sourceCount);
		expect(engine.getStreamingMetrics().branches.b.uniqueBuffers).toBe(0);
		expect(engine.getStreamingMetrics().overall.uniqueBuffers).toBe(
			ownedBefore,
		);
	});

	it("keeps the paused frame when playing A/B preparation completes late", async () => {
		// Given
		const context = new FakeAudioContext();
		const reference = deferredDescriptor();
		const candidate = deferredDescriptor();
		transport.open
			.mockResolvedValueOnce(descriptor("old"))
			.mockReturnValueOnce(reference.promise)
			.mockReturnValueOnce(candidate.promise);
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();
		context.currentTime = 0.3;
		const preparation = engine.prepareAB("project-1", "proposal-1");
		await Promise.resolve();

		// When
		engine.pause();
		const pausedPosition = engine.getState().positionBeat;
		reference.release(descriptor("a"));
		candidate.release(descriptor("b"));
		await preparation;

		// Then
		expect(engine.getState().status).toBe("paused");
		expect(engine.getState().positionBeat).toBe(pausedPosition);
		expect(engine.getStreamingMetrics().branches.b.uniqueBuffers).toBe(0);
	});

	it("keeps an explicit seek instead of installing a late playing replacement", async () => {
		// Given
		const context = new FakeAudioContext();
		const reference = deferredDescriptor();
		const candidate = deferredDescriptor();
		transport.open
			.mockResolvedValueOnce(descriptor("old"))
			.mockReturnValueOnce(reference.promise)
			.mockReturnValueOnce(candidate.promise);
		transport.chunk.mockImplementation(
			(
				_context,
				_render,
				range: { readonly startFrame: number; readonly endFrame: number },
			) => Promise.resolve(new FakeBuffer(range.endFrame - range.startFrame)),
		);
		const engine = createEngine(context);
		engine.setSnapshot(snapshot());
		await engine.prepare("project-1");
		await engine.play();
		const preparation = engine.prepareAB("project-1", "proposal-1");
		await Promise.resolve();

		// When
		engine.seek(5);
		reference.release(descriptor("a"));
		candidate.release(descriptor("b"));
		await preparation;
		await flush();

		// Then
		expect(engine.getState().positionBeat).toBe(5);
		expect(engine.getStreamingMetrics().branches.b.uniqueBuffers).toBe(0);
	});
});
