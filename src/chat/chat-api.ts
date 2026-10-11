import ky, { HTTPError } from "ky";
import { z } from "zod";
import {
	ApiErrorSchema,
	AuthStatusSchema,
	CreateSessionResponseSchema,
	CreateTurnResponseSchema,
	EmptyResponseSchema,
	LoginResponseSchema,
	LoginStatusSchema,
	LogoutResponseSchema,
	ModelsResponseSchema,
	SelectAccountResponseSchema,
	SessionResponseSchema,
	SessionsResponseSchema,
} from "./chat-contracts";

export class ChatApiError extends Error {
	readonly name = "ChatApiError";
	constructor(
		readonly code: string,
		message: string,
		options?: ErrorOptions,
	) {
		super(message, options);
	}
}

const chatHttp = ky.create({
	credentials: "same-origin",
	timeout: 20_000,
	retry: { limit: 1, methods: ["get"] },
});

type RequestOptions = {
	readonly method?: "get" | "post";
	readonly json?: Readonly<Record<string, unknown>>;
};

const requestChat = async <Schema extends z.ZodType>(
	path: string,
	schema: Schema,
	options: RequestOptions = {},
): Promise<z.output<Schema>> => {
	try {
		const response = await chatHttp(path, {
			method: options.method ?? "get",
			...(options.json === undefined ? {} : { json: options.json }),
		}).json();
		return schema.parse(response);
	} catch (error) {
		if (error instanceof HTTPError) {
			const parsed = ApiErrorSchema.safeParse(
				await error.response
					.clone()
					.json()
					.catch(() => null),
			);
			throw new ChatApiError(
				parsed.success ? parsed.data.error.code : "ChatHttpFailure",
				parsed.success
					? parsed.data.error.message
					: `Chat request failed with status ${error.response.status}.`,
				{ cause: error },
			);
		}
		if (error instanceof z.ZodError) {
			throw new ChatApiError(
				"InvalidChatResponse",
				"The chat service returned data this Studio version cannot read.",
				{ cause: error },
			);
		}
		throw error;
	}
};

const pathWith = (
	path: string,
	params: Readonly<Record<string, string>>,
): string => {
	const query = new URLSearchParams(params);
	return `${path}?${query.toString()}`;
};

export const getAuthStatus = () =>
	requestChat("api/chat/auth/status", AuthStatusSchema);
export const startLogin = (accountId?: string, mode?: "auto" | "paste") =>
	requestChat("api/chat/auth/login", LoginResponseSchema, {
		method: "post",
		json: {
			...(accountId === undefined ? {} : { accountId }),
			...(mode === undefined ? {} : { mode }),
		},
	});
export const getLoginStatus = (loginId: string) =>
	requestChat(
		`api/chat/auth/login/${encodeURIComponent(loginId)}`,
		LoginStatusSchema,
	);
export const completeLogin = (loginId: string, callbackUrl: string) =>
	requestChat("api/chat/auth/login/complete", EmptyResponseSchema, {
		method: "post",
		json: { loginId, callbackUrl },
	});
export const logoutAccount = (accountId: string) =>
	requestChat("api/chat/auth/logout", LogoutResponseSchema, {
		method: "post",
		json: { accountId },
	});
export const selectAccount = (accountId: string) =>
	requestChat("api/chat/auth/select", SelectAccountResponseSchema, {
		method: "post",
		json: { accountId },
	});
export const getModels = (accountId: string) =>
	requestChat(pathWith("api/chat/models", { accountId }), ModelsResponseSchema);
export const getSessions = (projectId: string, accountId: string) =>
	requestChat(
		pathWith("api/chat/sessions", { projectId, accountId }),
		SessionsResponseSchema,
	);
export const createSession = (
	projectId: string,
	accountId: string,
	model: string,
) =>
	requestChat("api/chat/sessions", CreateSessionResponseSchema, {
		method: "post",
		json: { projectId, accountId, model },
	});
export const getSession = (sessionId: string) =>
	requestChat(
		`api/chat/sessions/${encodeURIComponent(sessionId)}`,
		SessionResponseSchema,
	);
export const createTurn = (sessionId: string, text: string) =>
	requestChat(
		`api/chat/sessions/${encodeURIComponent(sessionId)}/turns`,
		CreateTurnResponseSchema,
		{
			method: "post",
			json: { clientTurnId: crypto.randomUUID(), text },
		},
	);
export const cancelTurn = (sessionId: string, turnId: string) =>
	requestChat(
		`api/chat/sessions/${encodeURIComponent(sessionId)}/turns/${encodeURIComponent(turnId)}/cancel`,
		EmptyResponseSchema,
		{ method: "post", json: {} },
	);
