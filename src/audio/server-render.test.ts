import { beforeEach, describe, expect, it, vi } from "vitest";
import {
	CHUNK_FRAMES,
	fetchPcmChunk,
	openServerRender,
	ServerRenderError,
} from "./server-render";

const http = vi.hoisted(() => ({ get: vi.fn() }));
vi.mock("ky", () => ({ default: { get: http.get } }));

const INFO = {
	format: "daiw-wav-pcm16-v1",
	revision: "revision-1",
	renderKey: "render-key",
	sampleRate: 48_000,
	channels: 2,
	frames: 48_001,
	byteLength: 192_048,
	dataOffset: 44,
	bytesPerFrame: 4,
	sha256: "a".repeat(64),
	pcmRms: 0.25,
	pcmPeak: 0.75,
} as const;

function response(body: BodyInit | null, init: ResponseInit): Response {
	return new Response(body, init);
}

describe("bounded server render transport", () => {
	beforeEach(() => http.get.mockReset());

	it("opens strict render metadata with required revision provenance", async () => {
		// Given
		http.get.mockResolvedValue(
			response(JSON.stringify(INFO), {
				status: 200,
				headers: {
					"Content-Type": "application/json",
					ETag: '"render-key"',
					"X-DAIW-Revision": "revision-1",
					"X-DAIW-SHA256": INFO.sha256,
				},
			}),
		);

		// When
		const descriptor = await openServerRender({
			projectId: "project 1",
			expectedRevision: "revision-1",
		});

		// Then
		expect(descriptor.info).toEqual(INFO);
		expect(http.get).toHaveBeenCalledWith(
			"/api/projects/project%201/render-info",
			expect.objectContaining({
				searchParams: new URLSearchParams({ expectedRevision: "revision-1" }),
			}),
		);
	});

	it("rejects metadata whose identity headers disagree with its body", async () => {
		// Given
		http.get.mockResolvedValue(
			response(JSON.stringify(INFO), {
				status: 200,
				headers: {
					ETag: '"other-key"',
					"X-DAIW-Revision": "revision-1",
					"X-DAIW-SHA256": INFO.sha256,
				},
			}),
		);

		// When
		const opening = openServerRender({
			projectId: "p",
			expectedRevision: "revision-1",
		});

		// Then
		await expect(opening).rejects.toBeInstanceOf(ServerRenderError);
	});

	it("requests at most one 500ms PCM range and decodes signed little-endian samples manually", async () => {
		// Given
		const bytes = new Uint8Array([
			0x00, 0x80, 0xff, 0x7f, 0x00, 0x00, 0x00, 0xc0,
		]);
		http.get
			.mockResolvedValueOnce(
				response(JSON.stringify(INFO), {
					status: 200,
					headers: {
						ETag: '"render-key"',
						"X-DAIW-Revision": "revision-1",
						"X-DAIW-SHA256": INFO.sha256,
					},
				}),
			)
			.mockResolvedValueOnce(
				response(bytes, {
					status: 206,
					headers: {
						"Content-Range": "bytes 44-51/192048",
						"Content-Length": "8",
						"Content-Encoding": "identity",
						ETag: '"render-key"',
						"X-DAIW-Revision": "revision-1",
						"X-DAIW-SHA256": INFO.sha256,
					},
				}),
			);
		const channels = [new Float32Array(2), new Float32Array(2)];
		const context = {
			createBuffer: vi.fn(() => ({
				duration: 2 / 48_000,
				length: 2,
				numberOfChannels: 2,
				sampleRate: 48_000,
				getChannelData: (channel: number) =>
					channels[channel] ?? new Float32Array(),
				copyFromChannel: vi.fn(),
				copyToChannel: vi.fn(),
			})),
		};
		const descriptor = await openServerRender({
			projectId: "p",
			expectedRevision: "revision-1",
		});

		// When
		const buffer = await fetchPcmChunk(context, descriptor, {
			startFrame: 0,
			endFrame: 2,
			signal: new AbortController().signal,
		});

		// Then
		expect(context.createBuffer).toHaveBeenCalledWith(2, 2, 48_000);
		expect(channels[0]).toEqual(new Float32Array([-1, 0]));
		expect(channels[1]).toEqual(new Float32Array([32_767 / 32_768, -0.5]));
		expect(buffer).toBeDefined();
		expect(http.get).toHaveBeenLastCalledWith(
			"/api/projects/p/render.wav",
			expect.objectContaining({
				searchParams: new URLSearchParams({
					expectedRevision: "revision-1",
					renderKey: "render-key",
				}),
				headers: { Range: "bytes=44-51" },
			}),
		);
		expect(CHUNK_FRAMES).toBe(24_000);
	});

	it("rejects full-response fallback before reading PCM bytes", async () => {
		// Given
		const arrayBuffer = vi.fn();
		http.get.mockResolvedValue({
			status: 200,
			headers: new Headers(),
			arrayBuffer,
		});
		const descriptor = {
			info: INFO,
			wavUrl: "/render.wav",
			searchParams: new URLSearchParams(),
		};

		// When
		const fetching = fetchPcmChunk({ createBuffer: vi.fn() }, descriptor, {
			startFrame: 0,
			endFrame: CHUNK_FRAMES,
			signal: new AbortController().signal,
		});

		// Then
		await expect(fetching).rejects.toBeInstanceOf(ServerRenderError);
		expect(arrayBuffer).not.toHaveBeenCalled();
	});
});
