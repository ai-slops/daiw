import type {
	ChatEvent,
	ChatSession,
	ChatTool,
	ChatTurn,
} from "./chat-contracts";

export type ChatScope = {
	readonly projectId: string;
	readonly accountId: string;
};

type StreamTurn = ChatTurn;
export type ChatState = {
	readonly scope: ChatScope | null;
	readonly sessionId: ChatSession["id"] | null;
	readonly turns: readonly StreamTurn[];
};

export const initialChatState: ChatState = {
	scope: null,
	sessionId: null,
	turns: [],
};

export type ChatAction =
	| { readonly type: "scope.changed"; readonly scope: ChatScope | null }
	| {
			readonly type: "session.loaded";
			readonly scope: ChatScope;
			readonly sessionId: ChatSession["id"];
			readonly turns: readonly ChatTurn[];
	  }
	| {
			readonly type: "session.refreshed";
			readonly scope: ChatScope;
			readonly sessionId: ChatSession["id"];
			readonly turns: readonly ChatTurn[];
	  }
	| {
			readonly type: "turn.started";
			readonly scope: ChatScope;
			readonly sessionId: ChatSession["id"];
			readonly turnId: ChatTurn["id"];
			readonly text: string;
	  }
	| {
			readonly type: "stream.event";
			readonly scope: ChatScope;
			readonly sessionId: ChatSession["id"];
			readonly turnId: ChatTurn["id"];
			readonly eventId: number;
			readonly event: ChatEvent;
	  };

const sameScope = (left: ChatScope | null, right: ChatScope | null): boolean =>
	left === null || right === null
		? left === right
		: left.projectId === right.projectId && left.accountId === right.accountId;

const mergeTool = (tools: readonly ChatTool[], next: ChatTool): ChatTool[] => {
	const exists = tools.some((tool) => tool.callId === next.callId);
	return exists
		? tools.map((tool) => (tool.callId === next.callId ? next : tool))
		: [...tools, next];
};

const applyEvent = (turn: StreamTurn, event: ChatEvent): StreamTurn => {
	switch (event.type) {
		case "text.delta":
			return { ...turn, assistantText: turn.assistantText + event.text };
		case "tool.started":
			return {
				...turn,
				tools: mergeTool(turn.tools, {
					callId: event.callId,
					name: event.name,
					status: "running",
				}),
			};
		case "tool.completed":
			return {
				...turn,
				tools: mergeTool(turn.tools, {
					callId: event.callId,
					name: event.name,
					status: "completed",
					result: event.result,
				}),
			};
		case "turn.completed":
			return { ...turn, status: "completed" };
		case "turn.failed":
			return {
				...turn,
				status: "failed",
				...(event.error === undefined ? {} : { error: event.error }),
			};
		case "turn.cancelled":
			return { ...turn, status: "cancelled" };
		case "turn.interrupted":
			return {
				...turn,
				status: "interrupted",
				...(event.error === undefined ? {} : { error: event.error }),
			};
		default:
			return event satisfies never;
	}
};

export const chatReducer = (
	state: ChatState,
	action: ChatAction,
): ChatState => {
	switch (action.type) {
		case "scope.changed":
			return sameScope(state.scope, action.scope)
				? state
				: { scope: action.scope, sessionId: null, turns: [] };
		case "session.loaded":
			if (!sameScope(state.scope, action.scope)) return state;
			return {
				...state,
				sessionId: action.sessionId,
				turns: action.turns,
			};
		case "session.refreshed":
			if (
				!sameScope(state.scope, action.scope) ||
				state.sessionId !== action.sessionId
			)
				return state;
			return { ...state, turns: action.turns };
		case "turn.started": {
			if (state.scope !== null && !sameScope(state.scope, action.scope))
				return state;
			const turn: StreamTurn = {
				id: action.turnId,
				text: action.text,
				status: "running",
				assistantText: "",
				lastEventId: 0,
				tools: [],
			};
			return {
				scope: action.scope,
				sessionId: action.sessionId,
				turns: [...state.turns, turn],
			};
		}
		case "stream.event":
			if (
				!sameScope(state.scope, action.scope) ||
				state.sessionId !== action.sessionId
			)
				return state;
			return {
				...state,
				turns: state.turns.map((turn) => {
					if (turn.id !== action.turnId) return turn;
					if (turn.status !== "running" || action.eventId <= turn.lastEventId)
						return turn;
					return {
						...applyEvent(turn, action.event),
						lastEventId: action.eventId,
					};
				}),
			};
		default:
			return action satisfies never;
	}
};
