import { beforeEach, describe, expect, it, vi } from "vitest";
import { getSession } from "./chat-api";
import {
	AccountIdSchema,
	SessionIdSchema,
	TurnIdSchema,
} from "./chat-contracts";
import { connectChatStream, type StreamTarget } from "./use-chat-stream";

vi.mock("./chat-api", () => ({ getSession: vi.fn() }));

class FakeEventSource {
	static latest: FakeEventSource | undefined;
	readonly listeners = new Map<string, EventListener>();
	readonly close = vi.fn();
	onopen: (() => void) | null = null;
	onerror: (() => void) | null = null;

	constructor(
		readonly url: URL,
		readonly options: EventSourceInit,
	) {
		FakeEventSource.latest = this;
	}

	addEventListener(type: string, listener: EventListener): void {
		this.listeners.set(type, listener);
	}

	emit(type: string, data: object, eventId: string): void {
		this.listeners.get(type)?.(
			new MessageEvent(type, {
				data: JSON.stringify(data),
				lastEventId: eventId,
			}),
		);
	}
}

const target: StreamTarget = {
	eventsUrl: "/api/chat/events",
	scope: {
		projectId: "project-1",
		accountId: AccountIdSchema.parse("account-1"),
	},
	sessionId: SessionIdSchema.parse("session-1"),
	turnId: TurnIdSchema.parse("turn-1"),
	lastEventId: 4,
};

describe("connectChatStream", () => {
	beforeEach(() => {
		FakeEventSource.latest = undefined;
		vi.clearAllMocks();
		vi.stubGlobal("window", { location: { origin: "https://studio.test" } });
		vi.stubGlobal("EventSource", FakeEventSource);
	});

	it("streams proposal and terminal events before loading canonical turns", async () => {
		const dispatch = vi.fn();
		const onConnectedChange = vi.fn();
		const onError = vi.fn();
		const onOpen = vi.fn();
		const onProjectChanged = vi.fn();
		vi.mocked(getSession).mockResolvedValue({
			session: {
				id: target.sessionId,
				projectId: target.scope.projectId,
				accountId: AccountIdSchema.parse(target.scope.accountId),
				model: "gpt-test",
				title: "Test",
				updatedAt: "2026-10-11T00:00:00Z",
			},
			turns: [],
		});

		connectChatStream(target, {
			dispatch,
			onConnectedChange,
			onError,
			onOpen,
			onProjectChanged,
		});
		const source = FakeEventSource.latest;
		expect(source?.url.href).toBe(
			"https://studio.test/api/chat/events?after=4",
		);
		expect(source?.options).toEqual({ withCredentials: true });
		source?.onopen?.();
		source?.emit(
			"tool.completed",
			{
				callId: "call-1",
				name: "proposal_create",
				result: { proposalId: "proposal-1" },
			},
			"5",
		);
		source?.emit("turn.completed", { turnId: target.turnId }, "6");

		expect(onOpen).toHaveBeenCalledOnce();
		expect(onConnectedChange).toHaveBeenNthCalledWith(1, true);
		expect(onConnectedChange).toHaveBeenNthCalledWith(2, false);
		expect(onProjectChanged).toHaveBeenCalledWith("project-1");
		expect(source?.close).toHaveBeenCalledOnce();
		await vi.waitFor(() => expect(dispatch).toHaveBeenCalledTimes(3));
		expect(dispatch.mock.calls[2]?.[0]).toMatchObject({
			type: "session.refreshed",
			sessionId: target.sessionId,
			turns: [],
		});
		expect(onError).not.toHaveBeenCalled();
	});

	it("ignores every retained callback after the connection is closed", async () => {
		// Given an attached stream whose EventSource callbacks are retained
		const dispatch = vi.fn();
		const onConnectedChange = vi.fn();
		const onError = vi.fn();
		const onOpen = vi.fn();
		const onProjectChanged = vi.fn();
		const connection = connectChatStream(target, {
			dispatch,
			onConnectedChange,
			onError,
			onOpen,
			onProjectChanged,
		});
		const source = FakeEventSource.latest;

		// When the connection closes before late browser callbacks run
		connection?.close();
		source?.onopen?.();
		source?.onerror?.();
		source?.emit(
			"tool.completed",
			{
				callId: "call-late",
				name: "proposal_create",
				result: { proposalId: "proposal-late" },
			},
			"5",
		);
		source?.emit("turn.completed", { turnId: target.turnId }, "6");
		await Promise.resolve();

		// Then stale callbacks have no observable effects
		expect(dispatch).not.toHaveBeenCalled();
		expect(onConnectedChange).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(onOpen).not.toHaveBeenCalled();
		expect(onProjectChanged).not.toHaveBeenCalled();
		expect(getSession).not.toHaveBeenCalled();
	});

	it("ignores a terminal refresh that resolves after the connection closes", async () => {
		// Given a terminal event with its canonical refresh still pending
		const dispatch = vi.fn();
		let resolveSession:
			| ((value: Awaited<ReturnType<typeof getSession>>) => void)
			| undefined;
		vi.mocked(getSession).mockReturnValue(
			new Promise((resolve) => {
				resolveSession = resolve;
			}),
		);
		const connection = connectChatStream(target, {
			dispatch,
			onConnectedChange: vi.fn(),
			onError: vi.fn(),
			onOpen: vi.fn(),
			onProjectChanged: vi.fn(),
		});
		FakeEventSource.latest?.emit(
			"turn.completed",
			{ turnId: target.turnId },
			"5",
		);

		// When another lifecycle closes the connection before refresh completion
		connection?.close();
		resolveSession?.({
			session: {
				id: target.sessionId,
				projectId: target.scope.projectId,
				accountId: AccountIdSchema.parse(target.scope.accountId),
				model: "gpt-test",
				title: "Test",
				updatedAt: "2026-10-11T00:00:00Z",
			},
			turns: [],
		});
		await Promise.resolve();

		// Then only the terminal event was dispatched
		expect(dispatch).toHaveBeenCalledTimes(1);
		expect(dispatch).toHaveBeenLastCalledWith(
			expect.objectContaining({ type: "stream.event" }),
		);
	});
});
