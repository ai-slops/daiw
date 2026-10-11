import { describe, expect, it } from "vitest";
import {
	completionModeForHostname,
	createCallbackUrlSchema,
	parseAuthorizationUrl,
} from "./auth-window";

const redirectUri = "http://127.0.0.1:43123/auth/callback";
const authorizationUrl =
	"https://auth.openai.com/api/accounts/authorize?client_id=daiw&state=opaque";

describe("completionModeForHostname", () => {
	it.each(["localhost", "127.0.0.1", "[::1]"])(
		"uses automatic loopback completion on %s",
		(hostname) => {
			expect(completionModeForHostname(hostname)).toBe("auto");
		},
	);

	it("uses paste completion for a remote Studio origin", () => {
		expect(completionModeForHostname("studio.example.com")).toBe("paste");
	});
});

describe("parseAuthorizationUrl", () => {
	it("accepts the official OpenAI account authorization endpoint", () => {
		const value =
			"https://auth.openai.com/api/accounts/authorize?client_id=daiw&state=opaque";

		expect(parseAuthorizationUrl(value)).toBe(value);
	});

	it.each([
		"https://auth.openai.com/oauth/authorize",
		"https://auth.openai.com:444/api/accounts/authorize",
		"https://user@auth.openai.com/api/accounts/authorize",
		"https://auth.openai.com/api/accounts/authorize#fragment",
	])("rejects an unsafe authorization URL: %s", (value) => {
		expect(() => parseAuthorizationUrl(value)).toThrow();
	});
});

describe("createCallbackUrlSchema", () => {
	it.each([
		"http://127.0.0.1:43123/auth/callback?code=secret&state=opaque",
		"http://127.0.0.1:43123/auth/callback?error=access_denied&state=opaque",
	])("accepts a backend-compatible callback URL: %s", (value) => {
		const schema = createCallbackUrlSchema(redirectUri, authorizationUrl);

		expect(schema.safeParse(value)).toEqual({ success: true, data: value });
	});

	it("returns a validation failure instead of throwing for malformed input", () => {
		const schema = createCallbackUrlSchema(redirectUri, authorizationUrl);

		expect(() => schema.safeParse("bad")).not.toThrow();
		expect(schema.safeParse("bad").success).toBe(false);
	});

	it.each([
		"https://127.0.0.1:43123/auth/callback?code=secret&state=opaque",
		"http://localhost:43123/auth/callback?code=secret&state=opaque",
		"http://127.0.0.1:44444/auth/callback?code=secret&state=opaque",
		"http://127.0.0.1:43123/wrong?code=secret&state=opaque",
		"http://user@127.0.0.1:43123/auth/callback?code=secret&state=opaque",
		"http://127.0.0.1:43123/auth/callback?code=secret&state=opaque#fragment",
		"http://127.0.0.1:43123/auth/callback?code=secret",
		"http://127.0.0.1:43123/auth/callback?code=secret&state=wrong",
		"http://127.0.0.1:43123/auth/callback?code=secret&state=opaque&state=opaque",
		"http://127.0.0.1:43123/auth/callback?state=opaque",
		"http://127.0.0.1:43123/auth/callback?code=&state=opaque",
		"http://127.0.0.1:43123/auth/callback?code=one&code=two&state=opaque",
		"http://127.0.0.1:43123/auth/callback?error=&state=opaque",
	])("rejects a callback URL the backend cannot accept: %s", (value) => {
		const schema = createCallbackUrlSchema(redirectUri, authorizationUrl);

		expect(schema.safeParse(value).success).toBe(false);
	});
});
