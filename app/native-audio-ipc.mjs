import { z } from "zod";

const tokenSchema = z.string().min(20).max(128);
const envelope = {
	channel: z.literal("native-audio"),
	id: z.uuid(),
	clientId: z.string().min(1).max(80),
};
const session = { sessionToken: tokenSchema };
const requestSchema = z.discriminatedUnion("method", [
	z
		.object({
			...envelope,
			method: z.literal("capabilities"),
			args: z.object({}).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("listDevices"),
			args: z.object({}).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("prepare"),
			args: z
				.object({
					projectId: z.string().min(1).max(160),
					expectedRevision: z.string().min(1),
					deviceToken: tokenSchema.optional(),
				})
				.strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("play"),
			args: z.object(session).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("pause"),
			args: z.object(session).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("stop"),
			args: z.object(session).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("state"),
			args: z.object(session).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("seek"),
			args: z
				.object({ ...session, frame: z.number().int().nonnegative() })
				.strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("setLoop"),
			args: z
				.object({
					...session,
					loop: z
						.object({
							start: z.number().int().nonnegative(),
							end: z.number().int().positive(),
						})
						.strict()
						.nullable(),
				})
				.strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("release"),
			args: z.object(session).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("cancel"),
			args: z.object({ requestId: z.uuid() }).strict(),
		})
		.strict(),
	z
		.object({
			...envelope,
			method: z.literal("releaseClient"),
			args: z.object({}).strict(),
		})
		.strict(),
]);

export function attachNativeAudioIpc(controller, hostProcess = process) {
	const send = (message) => hostProcess.send?.(message);
	const listener = async (input) => {
		let request;
		try {
			request = requestSchema.parse(input);
		} catch {
			return;
		}
		try {
			let result;
			switch (request.method) {
				case "capabilities":
					result = await controller.capabilities();
					break;
				case "listDevices":
					result = await controller.listDevices(request.clientId);
					break;
				case "prepare":
					result = await controller.prepare(
						request.clientId,
						request.args,
						request.id,
					);
					break;
				case "cancel":
					result = await controller.cancel(
						request.clientId,
						request.args.requestId,
					);
					break;
				case "release":
					result = await controller.release(
						request.clientId,
						request.args.sessionToken,
					);
					break;
				case "releaseClient":
					result = await controller.releaseClient(request.clientId);
					break;
				case "seek":
					result = await controller.command(
						request.clientId,
						request.args.sessionToken,
						{ type: "seek", frame: request.args.frame },
					);
					break;
				case "setLoop":
					result = await controller.command(
						request.clientId,
						request.args.sessionToken,
						{ type: "setLoop", loop: request.args.loop },
					);
					break;
				case "play":
				case "pause":
				case "stop":
				case "state":
					result = await controller.command(
						request.clientId,
						request.args.sessionToken,
						{ type: request.method },
					);
					break;
			}
			send({
				channel: "native-audio",
				responseTo: request.id,
				ok: true,
				result: result ?? null,
			});
		} catch (error) {
			send({
				channel: "native-audio",
				responseTo: request.id,
				ok: false,
				error: {
					code:
						typeof error?.code === "string" ? error.code : "NativeAudioError",
					message:
						error instanceof Error
							? error.message
							: "native audio request failed",
				},
			});
		}
	};
	hostProcess.on("message", listener);
	return () => hostProcess.off("message", listener);
}
