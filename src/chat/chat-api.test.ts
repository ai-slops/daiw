import { beforeEach, describe, expect, it, vi } from "vitest";

const { request } = vi.hoisted(() => ({
	request: vi.fn(() => ({
		json: vi.fn().mockResolvedValue({ status: "pending" }),
	})),
}));

vi.mock("ky", () => ({
	default: { create: () => request },
	HTTPError: class HTTPError extends Error {},
}));

import { getLoginStatus } from "./chat-api";

describe("getLoginStatus", () => {
	beforeEach(() => request.mockClear());

	it("polls the specific login attempt instead of global account status", async () => {
		const result = await getLoginStatus("login-new");

		expect(request).toHaveBeenCalledWith(
			"api/chat/auth/login/login-new",
			expect.objectContaining({ method: "get" }),
		);
		expect(result).toEqual({ status: "pending" });
	});
});
