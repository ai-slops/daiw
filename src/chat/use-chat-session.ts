import {
	useCallback,
	useEffect,
	useMemo,
	useReducer,
	useRef,
	useState,
} from "react";
import {
	ChatApiError,
	cancelTurn,
	createSession,
	createTurn,
	getModels,
	getSession,
	getSessions,
} from "./chat-api";
import type { ChatSession } from "./chat-contracts";
import { ChatRequestGeneration } from "./chat-request-generation";
import { type ChatScope, chatReducer, initialChatState } from "./chat-state";
import { useChatStream } from "./use-chat-stream";

const errorMessage = (error: unknown): string => {
	if (error instanceof ChatApiError) {
		if (/auth|login|scope|token|expired|disconnected/i.test(error.code))
			return `${error.message} Sign in again or choose another account.`;
		if (/limit|quota|usage/i.test(error.code))
			return `${error.message} Check the account usage limit or choose another model.`;
		return error.message;
	}
	if (error instanceof Error) return error.message;
	return "The chat connection failed.";
};

type SessionOptions = {
	readonly projectId: string;
	readonly accountId: string | null;
	readonly onProjectChanged: (projectId: string) => void;
};

export const useChatSession = (options: SessionOptions) => {
	const [state, dispatch] = useReducer(chatReducer, initialChatState);
	const [models, setModels] = useState<
		readonly { readonly id: string; readonly displayName: string }[]
	>([]);
	const [sessions, setSessions] = useState<readonly ChatSession[]>([]);
	const [model, setModel] = useState("");
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string>();
	const scope = useMemo<ChatScope | null>(
		() =>
			options.accountId === null
				? null
				: { projectId: options.projectId, accountId: options.accountId },
		[options.accountId, options.projectId],
	);
	const requestsRef = useRef<ChatRequestGeneration | null>(null);
	requestsRef.current ??= new ChatRequestGeneration(scope);
	const requests = requestsRef.current;
	requests.sync(scope);
	const stateRef = useRef(state);
	stateRef.current = state;

	const onStreamError = useCallback(
		(cause: unknown) => setError(errorMessage(cause)),
		[],
	);
	const onStreamOpen = useCallback(() => setError(undefined), []);
	const { attach, close, connected } = useChatStream({
		dispatch,
		onError: onStreamError,
		onOpen: onStreamOpen,
		onProjectChanged: options.onProjectChanged,
	});

	useEffect(() => {
		const ticket = requests.scopeTicket();
		close();
		setModels([]);
		setSessions([]);
		setModel("");
		setError(undefined);
		dispatch({ type: "scope.changed", scope });
		if (scope === null) {
			setLoading(false);
			return;
		}
		setLoading(true);
		void Promise.all([
			getModels(scope.accountId),
			getSessions(scope.projectId, scope.accountId),
		])
			.then(([modelResult, sessionResult]) => {
				if (!requests.isScopeCurrent(ticket)) return;
				setModels(modelResult.models);
				setSessions(sessionResult.sessions);
				setModel(modelResult.models[0]?.id ?? "");
				setError(undefined);
			})
			.catch((cause: unknown) => {
				if (requests.isScopeCurrent(ticket)) setError(errorMessage(cause));
			})
			.finally(() => {
				if (requests.isScopeCurrent(ticket)) setLoading(false);
			});
	}, [close, requests, scope]);

	const open = useCallback(
		async (session: ChatSession) => {
			if (scope === null) return;
			const ticket = requests.beginSession();
			close();
			setLoading(true);
			try {
				const value = await getSession(session.id);
				if (!requests.isSessionCurrent(ticket)) return;
				dispatch({
					type: "session.loaded",
					scope,
					sessionId: value.session.id,
					turns: value.turns,
				});
				const running = value.turns.find((turn) => turn.status === "running");
				if (running !== undefined)
					attach({
						eventsUrl: `/api/chat/sessions/${session.id}/turns/${running.id}/events`,
						scope,
						sessionId: session.id,
						turnId: running.id,
						lastEventId: running.lastEventId,
					});
				setModel(session.model);
				setError(undefined);
			} catch (cause) {
				if (requests.isSessionCurrent(ticket)) setError(errorMessage(cause));
			} finally {
				if (requests.isSessionCurrent(ticket)) setLoading(false);
			}
		},
		[attach, close, requests, scope],
	);

	const startNew = useCallback(async () => {
		if (scope === null || model.length === 0) return;
		const ticket = requests.beginSession();
		close();
		setLoading(true);
		try {
			const value = await createSession(
				scope.projectId,
				scope.accountId,
				model,
			);
			if (!requests.isSessionCurrent(ticket)) return;
			setSessions((current) => [value.session, ...current]);
			dispatch({
				type: "session.loaded",
				scope,
				sessionId: value.session.id,
				turns: [],
			});
		} catch (cause) {
			if (requests.isSessionCurrent(ticket)) setError(errorMessage(cause));
		} finally {
			if (requests.isSessionCurrent(ticket)) setLoading(false);
		}
	}, [close, model, requests, scope]);

	const send = useCallback(
		async (text: string) => {
			if (scope === null || state.sessionId === null) return;
			const ticket = requests.sessionTicket();
			const sessionId = state.sessionId;
			try {
				const value = await createTurn(sessionId, text);
				if (
					!requests.isSessionCurrent(ticket) ||
					stateRef.current.sessionId !== sessionId
				)
					return;
				dispatch({
					type: "turn.started",
					scope,
					sessionId,
					turnId: value.turnId,
					text,
				});
				attach({
					eventsUrl: value.eventsUrl,
					scope,
					sessionId,
					turnId: value.turnId,
					lastEventId: 0,
				});
			} catch (cause) {
				if (
					requests.isSessionCurrent(ticket) &&
					stateRef.current.sessionId === sessionId
				)
					setError(errorMessage(cause));
			}
		},
		[attach, requests, scope, state.sessionId],
	);

	const cancel = useCallback(async () => {
		const running = state.turns.find((turn) => turn.status === "running");
		if (state.sessionId === null || running === undefined) return;
		const ticket = requests.sessionTicket();
		const sessionId = state.sessionId;
		try {
			await cancelTurn(sessionId, running.id);
		} catch (cause) {
			if (
				requests.isSessionCurrent(ticket) &&
				stateRef.current.sessionId === sessionId
			)
				setError(errorMessage(cause));
		}
	}, [requests, state.sessionId, state.turns]);

	const reconnect = useCallback(() => {
		if (scope === null || state.sessionId === null) return;
		const running = state.turns.find((turn) => turn.status === "running");
		if (running !== undefined)
			attach({
				eventsUrl: `/api/chat/sessions/${state.sessionId}/turns/${running.id}/events`,
				scope,
				sessionId: state.sessionId,
				turnId: running.id,
				lastEventId: running.lastEventId,
			});
	}, [attach, scope, state.sessionId, state.turns]);

	return {
		state,
		models,
		sessions,
		model,
		setModel,
		loading,
		error,
		connected,
		open,
		startNew,
		send,
		cancel,
		reconnect,
	};
};
