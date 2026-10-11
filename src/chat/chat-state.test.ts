import { describe, expect, it } from "vitest";
import {
	parseChatEvent,
	SessionIdSchema,
	TurnIdSchema,
} from "./chat-contracts";
import { chatReducer, initialChatState } from "./chat-state";

const firstScope = { projectId: "project-a", accountId: "account-a" };
const secondScope = { projectId: "project-b", accountId: "account-b" };
const sessionId = SessionIdSchema.parse("session-a");
const turnId = TurnIdSchema.parse("turn-a");

describe("chatReducer", () => {
	it("ignores late stream events when account or project changes", () => {
		// Given a running turn in the first account and project scope
		const running = chatReducer(initialChatState, {
			type: "turn.started",
			scope: firstScope,
			sessionId,
			turnId,
			text: "Question",
		});
		const switched = chatReducer(running, {
			type: "scope.changed",
			scope: secondScope,
		});

		// When a delayed delta from the old stream arrives
		const result = chatReducer(switched, {
			type: "stream.event",
			scope: firstScope,
			sessionId,
			turnId,
			eventId: 4,
			event: parseChatEvent("text.delta", { text: "Late text" }),
		});

		// Then the new scope remains uncontaminated
		expect(result).toEqual(switched);
	});

	it("clears the selected session and transcript when scope becomes unavailable", () => {
		// Given a loaded chat in an authenticated scope
		const running = chatReducer(initialChatState, {
			type: "turn.started",
			scope: firstScope,
			sessionId,
			turnId,
			text: "Question",
		});

		// When logout removes the chat scope
		const result = chatReducer(running, {
			type: "scope.changed",
			scope: null,
		});

		// Then no scoped chat state survives
		expect(result).toEqual(initialChatState);
	});

	it("refreshes terminal turns only for the currently selected session", () => {
		// Given an explicitly selected session
		const selected = chatReducer(
			chatReducer(initialChatState, {
				type: "scope.changed",
				scope: firstScope,
			}),
			{
				type: "session.loaded",
				scope: firstScope,
				sessionId,
				turns: [],
			},
		);
		const otherSessionId = SessionIdSchema.parse("session-b");

		// When a terminal refresh from an older session resolves
		const result = chatReducer(selected, {
			type: "session.refreshed",
			scope: firstScope,
			sessionId: otherSessionId,
			turns: [
				{
					id: turnId,
					text: "Stale",
					status: "completed",
					assistantText: "Stale answer",
					lastEventId: 1,
					tools: [],
				},
			],
		});

		// Then it cannot replace the explicit selection
		expect(result).toEqual(selected);
	});

	it("deduplicates replayed event ids", () => {
		// Given a running turn with one streamed delta
		const running = chatReducer(initialChatState, {
			type: "turn.started",
			scope: firstScope,
			sessionId,
			turnId,
			text: "Question",
		});
		const action = {
			type: "stream.event",
			scope: firstScope,
			sessionId,
			turnId,
			eventId: 5,
			event: parseChatEvent("text.delta", { text: "Answer" }),
		} as const;
		const once = chatReducer(running, action);

		// When the same SSE event is replayed
		const replayed = chatReducer(once, action);

		// Then its text appears only once
		expect(replayed.turns[0]?.assistantText).toBe("Answer");
	});

	it("continues a partial snapshot after its persisted event cursor", () => {
		const scoped = chatReducer(initialChatState, {
			type: "scope.changed",
			scope: firstScope,
		});
		const restored = chatReducer(scoped, {
			type: "session.loaded",
			scope: firstScope,
			sessionId,
			turns: [
				{
					id: turnId,
					text: "Question",
					status: "running",
					assistantText: "Partial",
					lastEventId: 4,
					tools: [],
				},
			],
		});
		const replayed = chatReducer(restored, {
			type: "stream.event",
			scope: firstScope,
			sessionId,
			turnId,
			eventId: 4,
			event: parseChatEvent("text.delta", { text: "Partial" }),
		});

		const continued = chatReducer(replayed, {
			type: "stream.event",
			scope: firstScope,
			sessionId,
			turnId,
			eventId: 5,
			event: parseChatEvent("text.delta", { text: " answer" }),
		});

		expect(continued.turns[0]?.assistantText).toBe("Partial answer");
		expect(continued.turns[0]?.lastEventId).toBe(5);
	});

	it("ignores late events after a turn is cancelled", () => {
		const running = chatReducer(initialChatState, {
			type: "turn.started",
			scope: firstScope,
			sessionId,
			turnId,
			text: "Question",
		});
		const cancelled = chatReducer(running, {
			type: "stream.event",
			scope: firstScope,
			sessionId,
			turnId,
			eventId: 1,
			event: parseChatEvent("turn.cancelled", { turnId }),
		});

		const late = chatReducer(cancelled, {
			type: "stream.event",
			scope: firstScope,
			sessionId,
			turnId,
			eventId: 2,
			event: parseChatEvent("text.delta", { text: "Late" }),
		});

		expect(late).toEqual(cancelled);
	});
});

describe("parseChatEvent", () => {
	it("parses completed tool calls into typed tool cards", () => {
		// Given a tool completion payload from the SSE boundary
		const payload = {
			callId: "call-1",
			name: "proposal_create",
			result: { proposalId: "proposal-1", hunkCount: 2 },
		};

		// When the payload is parsed
		const event = parseChatEvent("tool.completed", payload);

		// Then the discriminant and structured result survive boundary parsing
		expect(event).toEqual({ type: "tool.completed", ...payload });
	});

	it("rejects unsupported tool names", () => {
		// Given an unapproved mutation tool
		const payload = { callId: "call-2", name: "proposal_accept" };

		// When and then it crosses the boundary, parsing fails
		expect(() => parseChatEvent("tool.started", payload)).toThrow();
	});
});
