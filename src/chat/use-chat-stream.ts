import { type Dispatch, useCallback, useEffect, useRef, useState } from "react";
import { z } from "zod";
import { getSession } from "./chat-api";
import type { ChatEventName, ChatSession, ChatTurn } from "./chat-contracts";
import { parseChatEventJson } from "./chat-contracts";
import type { ChatAction, ChatScope } from "./chat-state";

const eventNames = [
	"text.delta",
	"tool.started",
	"tool.completed",
	"turn.completed",
	"turn.failed",
	"turn.cancelled",
	"turn.interrupted",
] as const satisfies readonly ChatEventName[];
const terminalEvents = new Set<ChatEventName>([
	"turn.completed",
	"turn.failed",
	"turn.cancelled",
	"turn.interrupted",
]);

export type StreamTarget = {
	readonly eventsUrl: string;
	readonly scope: ChatScope;
	readonly sessionId: ChatSession["id"];
	readonly turnId: ChatTurn["id"];
	readonly lastEventId: number;
};

type StreamOptions = {
	readonly dispatch: Dispatch<ChatAction>;
	readonly onError: (error: unknown) => void;
	readonly onOpen: () => void;
	readonly onProjectChanged: (projectId: string) => void;
};

type StreamCallbacks = StreamOptions & {
	readonly onConnectedChange: (connected: boolean) => void;
};

type ChatStreamConnection = {
	readonly close: () => void;
};

export const connectChatStream = (
	target: StreamTarget,
	callbacks: StreamCallbacks,
): ChatStreamConnection | undefined => {
	const url = new URL(target.eventsUrl, window.location.origin);
	if (url.origin !== window.location.origin) {
		callbacks.onError(
			new TypeError("The chat service returned an unsafe event stream URL."),
		);
		return undefined;
	}
	url.searchParams.set("after", String(target.lastEventId));
	const source = new EventSource(url, { withCredentials: true });
	let active = true;
	let receiving = true;
	source.onopen = () => {
		if (!receiving) return;
		callbacks.onConnectedChange(true);
		callbacks.onOpen();
	};
	source.onerror = () => {
		if (!receiving) return;
		callbacks.onConnectedChange(false);
		callbacks.onError(
			new TypeError(
				"Live response disconnected. Reconnect to continue this turn.",
			),
		);
	};
	for (const name of eventNames) {
		source.addEventListener(name, (rawEvent) => {
			if (!receiving || !(rawEvent instanceof MessageEvent)) return;
			try {
				const eventId = z.coerce
					.number()
					.int()
					.positive()
					.parse(rawEvent.lastEventId);
				const event = parseChatEventJson(name, String(rawEvent.data));
				callbacks.dispatch({
					type: "stream.event",
					scope: target.scope,
					sessionId: target.sessionId,
					turnId: target.turnId,
					eventId,
					event,
				});
				if (event.type === "tool.completed" && event.name === "proposal_create")
					callbacks.onProjectChanged(target.scope.projectId);
				if (terminalEvents.has(name)) {
					receiving = false;
					source.close();
					callbacks.onConnectedChange(false);
					void getSession(target.sessionId)
						.then((latest) => {
							if (!active) return;
							callbacks.dispatch({
								type: "session.refreshed",
								scope: target.scope,
								sessionId: target.sessionId,
								turns: latest.turns,
							});
						})
						.catch((cause: unknown) => {
							if (active) callbacks.onError(cause);
						});
				}
			} catch (cause) {
				if (active) callbacks.onError(cause);
			}
		});
	}
	return {
		close: () => {
			active = false;
			receiving = false;
			source.close();
		},
	};
};

export const useChatStream = ({
	dispatch,
	onError,
	onOpen,
	onProjectChanged,
}: StreamOptions) => {
	const sourceRef = useRef<ChatStreamConnection | undefined>(undefined);
	const [connected, setConnected] = useState(false);
	const close = useCallback(() => {
		sourceRef.current?.close();
		sourceRef.current = undefined;
		setConnected(false);
	}, []);
	const attach = useCallback(
		(target: StreamTarget) => {
			close();
			sourceRef.current = connectChatStream(target, {
				dispatch,
				onConnectedChange: setConnected,
				onError,
				onOpen,
				onProjectChanged,
			});
		},
		[close, dispatch, onError, onOpen, onProjectChanged],
	);
	useEffect(() => close, [close]);
	return { attach, close, connected };
};
