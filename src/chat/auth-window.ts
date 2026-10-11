import { z } from "zod";

const AuthorizationUrlSchema = z.url().refine((value) => {
	const url = new URL(value);
	return (
		url.protocol === "https:" &&
		url.hostname === "auth.openai.com" &&
		url.pathname === "/api/accounts/authorize" &&
		url.port === "" &&
		url.username === "" &&
		url.password === "" &&
		url.hash === ""
	);
});
const OpenResultSchema = z.object({ opened: z.literal(true) });

type OpenAIBridge = Readonly<{
	openAuthorizationUrl: (url: string) => Promise<unknown>;
}>;

declare global {
	interface Window {
		readonly daiwOpenAI?: OpenAIBridge;
	}
}

export const completionModeForHostname = (
	hostname: string,
): "auto" | "paste" =>
	["localhost", "127.0.0.1", "[::1]"].includes(hostname) ? "auto" : "paste";

export const parseAuthorizationUrl = (value: string): string =>
	AuthorizationUrlSchema.parse(value);

export const createCallbackUrlSchema = (
	redirectUri: string,
	authorizationUrl: string,
) => {
	const expected = new URL(redirectUri);
	const expectedStates = new URL(authorizationUrl).searchParams.getAll("state");
	return z.string().superRefine((value, context) => {
		if (!URL.canParse(value)) {
			context.addIssue({
				code: "custom",
				message: "Paste the final 127.0.0.1 callback URL from your browser.",
			});
			return;
		}
		const callback = new URL(value);
		const states = callback.searchParams.getAll("state");
		const codes = callback.searchParams.getAll("code");
		const errors = callback.searchParams.getAll("error");
		const clientIds = callback.searchParams.getAll("client_id");
		const validOptionalParameter = (values: readonly string[]) =>
			values.length <= 1 && values[0] !== "";
		const validOutcome =
			(errors.length === 1 && errors[0] !== "") ||
			(codes.length === 1 && codes[0] !== "");
		const valid =
			callback.protocol === "http:" &&
			callback.hostname === "127.0.0.1" &&
			callback.host === expected.host &&
			callback.pathname === "/auth/callback" &&
			callback.username === "" &&
			callback.password === "" &&
			callback.hash === "" &&
			expectedStates.length === 1 &&
			states.length === 1 &&
			states[0] === expectedStates[0] &&
			validOptionalParameter(codes) &&
			validOptionalParameter(errors) &&
			validOptionalParameter(clientIds) &&
			validOutcome;
		if (!valid)
			context.addIssue({
				code: "custom",
				message: "Paste the final 127.0.0.1 callback URL from your browser.",
			});
	});
};

export const openPendingAuthWindow = (): Window | null => {
	const opened = window.open("about:blank", "daiw-chatgpt-auth");
	if (opened !== null) opened.opener = null;
	return opened;
};

export const navigateAuthWindow = async (
	value: string,
	opened: Window | null,
): Promise<boolean> => {
	const url = parseAuthorizationUrl(value);
	if (opened !== null) {
		opened.location.href = url;
		return true;
	}
	if (window.daiwOpenAI === undefined) return false;
	OpenResultSchema.parse(await window.daiwOpenAI.openAuthorizationUrl(url));
	return true;
};
