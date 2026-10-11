import { describe, expect, it } from "vitest";
import { canSendChat } from "./AssistantWorkspace";

describe("canSendChat", () => {
	it("requires consent, a matching scope, and a connected active account", () => {
		// Given consent and a matching selected chat scope
		const scope = { projectId: "project-a", accountId: "account-a" };

		// When each account connection state is evaluated
		const connected = canSendChat({
			approved: true,
			loading: false,
			projectId: "project-a",
			activeAccountId: "account-a",
			activeAccountConnected: true,
			scope,
		});
		const disconnected = canSendChat({
			approved: true,
			loading: false,
			projectId: "project-a",
			activeAccountId: "account-a",
			activeAccountConnected: false,
			scope,
		});
		const staleScope = canSendChat({
			approved: true,
			loading: false,
			projectId: "project-b",
			activeAccountId: "account-a",
			activeAccountConnected: true,
			scope,
		});

		// Then only the connected current scope can send
		expect(connected).toBe(true);
		expect(disconnected).toBe(false);
		expect(staleScope).toBe(false);
	});
});
