import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { type ChatRow, convertRow, TurnErrorAlert } from "./ChatThread";

describe("convertRow", () => {
	it("marks failed tool results as errors for the rendered tool card", () => {
		const row: ChatRow = {
			id: "assistant-1",
			role: "assistant",
			text: "",
			status: "failed",
			tools: [
				{
					callId: "call-1",
					name: "proposal_create",
					status: "failed",
					result: { code: "UnsupportedCapability" },
				},
			],
		};

		const message = convertRow(row);
		if (typeof message.content === "string")
			throw new TypeError("Assistant row must use structured content.");
		const tool = message.content.find((part) => part.type === "tool-call");

		expect(tool).toMatchObject({
			isError: true,
			result: { code: "UnsupportedCapability" },
		});
	});
});

describe("TurnErrorAlert", () => {
	it("renders a terminal error as escaped actionable alert text", () => {
		// Given a terminal turn error containing markup-like provider text
		const error = {
			code: "AccountDisconnected",
			message: "Reconnect <script>secret()</script>",
		};

		// When the inline error is rendered
		const markup = renderToStaticMarkup(
			createElement(TurnErrorAlert, {
				error,
				canRetry: true,
				onRetry: async () => undefined,
			}),
		);

		// Then its safe code and message are visible without executable markup
		expect(markup).toContain('role="alert"');
		expect(markup).toContain("AccountDisconnected");
		expect(markup).toContain("Reconnect &lt;script&gt;secret()&lt;/script&gt;");
		expect(markup).toContain("Sign in again or choose another account.");
		expect(markup).toContain("Try again");
		expect(markup).not.toContain("<script>");
	});
});
