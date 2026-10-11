import { describe, expect, it } from "vitest";
import {
	LoginStatusSchema,
	SessionResponseSchema,
	ToolNameSchema,
} from "./chat-contracts";

const session = {
	id: "session-1",
	projectId: "project-1",
	accountId: "account-1",
	model: "gpt-test",
	title: "Test",
	updatedAt: "2026-10-11T00:00:00Z",
};

describe("chat contracts", () => {
	it("accepts exactly the five production tool names", () => {
		expect(ToolNameSchema.options).toEqual([
			"project_context",
			"project_query_notes",
			"project_explain",
			"change_dry_run",
			"proposal_create",
		]);
	});

	it("restores the persisted event cursor with each turn", () => {
		const response = SessionResponseSchema.parse({
			session,
			turns: [
				{
					id: "turn-1",
					text: "Question",
					status: "running",
					assistantText: "Partial",
					lastEventId: 4,
					tools: [],
				},
			],
		});

		expect(response.turns[0]?.lastEventId).toBe(4);
	});

	it.each([
		{ status: "pending" },
		{ status: "completed", activeAccountId: "account-2" },
		{
			status: "failed",
			error: { code: "MissingScope", message: "Scope was not granted." },
		},
		{
			status: "expired",
			error: { code: "LoginExpired", message: "Login expired." },
		},
	])("parses login-attempt status $status", (value) => {
		expect(LoginStatusSchema.parse(value)).toEqual(value);
	});
});
