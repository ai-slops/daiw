import { z } from "zod";

const tokenSchema = z.string().min(20).max(128);
const capabilitiesSchema = z
	.object({
		available: z.boolean(),
		sampleRate: z.literal(48_000),
		channels: z.literal(2),
		ab: z.literal(false),
		meter: z.literal(false),
	})
	.strict();
const deviceSchema = z
	.object({
		token: tokenSchema,
		name: z.string(),
		isDefault: z.boolean(),
		supported: z.boolean(),
	})
	.strict();
const deviceListSchema = z
	.object({ generation: z.uuid(), devices: z.array(deviceSchema) })
	.strict();
const preparedSchema = z.object({ sessionToken: tokenSchema }).strict();
const workerStateSchema = z
	.object({
		status: z.enum(["stopped", "paused", "playing", "faulted"]),
		positionFrame: z.number().int().nonnegative(),
		frames: z.number().int().nonnegative(),
		loop: z
			.object({
				start: z.number().int().nonnegative(),
				end: z.number().int().positive(),
			})
			.strict()
			.nullable(),
		stats: z
			.object({
				underrunCallbacks: z.number().int().nonnegative(),
				staleBlocks: z.number().int().nonnegative(),
				deviceErrors: z.number().int().nonnegative(),
			})
			.strict(),
	})
	.strict();
const workerEventKeySchema = z.object({
	sessionToken: tokenSchema,
	projectId: z.string().min(1),
	expectedRevision: z.string().min(1),
});
const workerEventSchema = z.union([
	workerEventKeySchema.extend({ state: workerStateSchema }).strict(),
	workerEventKeySchema
		.extend({
			fault: z.object({ code: z.string(), message: z.string() }).strict(),
		})
		.strict(),
]);

export type NativeDevice = Readonly<z.infer<typeof deviceSchema>>;
export type NativeWorkerState = Readonly<z.infer<typeof workerStateSchema>>;
export type NativeWorkerEvent = Readonly<z.infer<typeof workerEventSchema>>;
export type NativeBridge = NonNullable<ReturnType<typeof nativeBridge>>;

type RawBridge = Readonly<{
	capabilities: () => Promise<unknown>;
	listDevices: () => Promise<unknown>;
	prepare: (
		request: Readonly<{
			projectId: string;
			expectedRevision: string;
			deviceToken?: string;
		}>,
	) => Promise<unknown>;
	play: (token: string) => Promise<unknown>;
	pause: (token: string) => Promise<unknown>;
	stop: (token: string) => Promise<unknown>;
	seek: (token: string, frame: number) => Promise<unknown>;
	setLoop: (
		token: string,
		loop: Readonly<{ start: number; end: number }> | null,
	) => Promise<unknown>;
	release: (token: string) => Promise<unknown>;
	state: (token: string) => Promise<unknown>;
	onState: (listener: (state: unknown) => void) => () => void;
}>;

declare global {
	interface Window {
		readonly daiwNativeAudio?: RawBridge;
	}
}

export const nativeBridgeAvailable = (): boolean =>
	window.daiwNativeAudio !== undefined;

export function nativeBridge() {
	const raw = window.daiwNativeAudio;
	if (raw === undefined) return undefined;
	return {
		capabilities: async () =>
			capabilitiesSchema.parse(await raw.capabilities()),
		listDevices: async () => deviceListSchema.parse(await raw.listDevices()),
		prepare: async (request: Parameters<RawBridge["prepare"]>[0]) =>
			preparedSchema.parse(await raw.prepare(request)),
		command: async (
			method: "play" | "pause" | "stop" | "state",
			sessionToken: string,
		) => workerStateSchema.parse(await raw[method](sessionToken)),
		seek: async (sessionToken: string, frame: number) =>
			workerStateSchema.parse(await raw.seek(sessionToken, frame)),
		setLoop: async (
			sessionToken: string,
			loop: Readonly<{ start: number; end: number }> | null,
		) => workerStateSchema.parse(await raw.setLoop(sessionToken, loop)),
		release: async (sessionToken: string) => {
			await raw.release(sessionToken);
		},
		onState: (
			listener: (event: NativeWorkerEvent) => void,
			invalid: (error: z.ZodError) => void,
		) =>
			raw.onState((input) => {
				const parsed = workerEventSchema.safeParse(input);
				if (parsed.success) listener(parsed.data);
				else invalid(parsed.error);
			}),
	};
}
