import { z } from "zod";

export const AccountIdSchema = z.string().min(1).brand("ChatAccountId");
export const SessionIdSchema = z.string().min(1).brand("ChatSessionId");
export const TurnIdSchema = z.string().min(1).brand("ChatTurnId");

const ErrorDetailSchema = z.object({ code: z.string(), message: z.string() });
export const ToolNameSchema = z.enum([
	"project_context",
	"project_query_notes",
	"project_explain",
	"change_dry_run",
	"proposal_create",
]);
const ToolStatusSchema = z.enum(["running", "completed", "failed"]);
export const ToolCallSchema = z.strictObject({
	callId: z.string().min(1),
	name: ToolNameSchema,
	status: ToolStatusSchema,
	result: z.record(z.string(), z.unknown()).optional(),
});
export const TurnSchema = z.object({
	id: TurnIdSchema,
	text: z.string(),
	status: z.enum([
		"running",
		"completed",
		"failed",
		"cancelled",
		"interrupted",
	]),
	assistantText: z.string(),
	lastEventId: z.number().int().nonnegative(),
	error: ErrorDetailSchema.optional(),
	tools: z.array(ToolCallSchema),
});
export const SessionSchema = z.object({
	id: SessionIdSchema,
	projectId: z.string().min(1),
	accountId: AccountIdSchema,
	model: z.string().min(1),
	title: z.string(),
	updatedAt: z.string(),
});

export const AuthStatusSchema = z.object({
	hostId: z.string(),
	callbackMode: z.enum(["auto", "paste"]),
	accounts: z.array(
		z.object({
			id: AccountIdSchema,
			connected: z.boolean(),
			profile: z.object({
				email: z.string().optional(),
				label: z.string(),
			}),
		}),
	),
	activeAccountId: AccountIdSchema.nullable(),
});
export const LoginResponseSchema = z.object({
	loginId: z.string().min(1),
	authorizationUrl: z.url().refine((value) => {
		const url = new URL(value);
		return url.protocol === "https:" && url.hostname === "auth.openai.com";
	}, "Authorization URL must use https://auth.openai.com"),
	redirectUri: z.url(),
	completionMode: z.enum(["auto", "paste"]),
	expiresAt: z.number(),
});
export const LoginStatusSchema = z.discriminatedUnion("status", [
	z.strictObject({ status: z.literal("pending") }),
	z.strictObject({
		status: z.literal("completed"),
		activeAccountId: AccountIdSchema,
	}),
	z.strictObject({ status: z.literal("failed"), error: ErrorDetailSchema }),
	z.strictObject({
		status: z.literal("expired"),
		error: ErrorDetailSchema.optional(),
	}),
]);
export const ModelsResponseSchema = z.object({
	models: z.array(z.object({ id: z.string().min(1), displayName: z.string() })),
});
export const SessionsResponseSchema = z.object({
	sessions: z.array(SessionSchema),
});
export const SessionResponseSchema = z.object({
	session: SessionSchema,
	turns: z.array(TurnSchema),
});
export const CreateSessionResponseSchema = z.object({ session: SessionSchema });
export const CreateTurnResponseSchema = z.object({
	turnId: TurnIdSchema,
	eventsUrl: z.string().min(1),
});
export const SelectAccountResponseSchema = z.object({
	activeAccountId: AccountIdSchema,
});
export const LogoutResponseSchema = z.object({
	revocationConfirmed: z.boolean(),
});
export const EmptyResponseSchema = z.record(z.string(), z.unknown());
export const ApiErrorSchema = z.object({ error: ErrorDetailSchema });

const ChatEventSchemas = {
	"text.delta": z.object({ type: z.literal("text.delta"), text: z.string() }),
	"tool.started": z.object({
		type: z.literal("tool.started"),
		callId: z.string().min(1),
		name: ToolNameSchema,
	}),
	"tool.completed": z.object({
		type: z.literal("tool.completed"),
		callId: z.string().min(1),
		name: ToolNameSchema,
		result: z.record(z.string(), z.unknown()),
	}),
	"turn.completed": z.object({
		type: z.literal("turn.completed"),
		turnId: TurnIdSchema,
	}),
	"turn.failed": z.object({
		type: z.literal("turn.failed"),
		turnId: TurnIdSchema,
		error: ErrorDetailSchema.optional(),
	}),
	"turn.cancelled": z.object({
		type: z.literal("turn.cancelled"),
		turnId: TurnIdSchema,
	}),
	"turn.interrupted": z.object({
		type: z.literal("turn.interrupted"),
		turnId: TurnIdSchema,
		error: ErrorDetailSchema.optional(),
	}),
} as const;

export type ChatEventName = keyof typeof ChatEventSchemas;
export type ChatEvent = z.infer<(typeof ChatEventSchemas)[ChatEventName]>;
export type ChatSession = z.infer<typeof SessionSchema>;
export type ChatTurn = z.infer<typeof TurnSchema>;
export type ChatTool = z.infer<typeof ToolCallSchema>;
export type AuthStatus = z.infer<typeof AuthStatusSchema>;
export type LoginStatus = z.infer<typeof LoginStatusSchema>;

export const parseChatEvent = (
	type: ChatEventName,
	payload: unknown,
): ChatEvent =>
	ChatEventSchemas[type].parse({
		type,
		...z.record(z.string(), z.unknown()).parse(payload),
	});

export const parseChatEventJson = (
	type: ChatEventName,
	payload: string,
): ChatEvent => parseChatEvent(type, z.json().parse(JSON.parse(payload)));
