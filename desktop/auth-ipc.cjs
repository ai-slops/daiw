const { isTrustedFrame } = require("./native-ipc.cjs");

const invalidAuthorizationUrl = (message) =>
	Object.assign(new Error(message), { code: "InvalidAuthorizationUrl" });

const parseAuthorizationUrl = (value) => {
	if (typeof value !== "string" || !URL.canParse(value))
		throw invalidAuthorizationUrl("Authorization URL is required and valid.");
	const url = new URL(value);
	if (
		url.protocol !== "https:" ||
		url.hostname !== "auth.openai.com" ||
		url.pathname !== "/api/accounts/authorize" ||
		url.port !== "" ||
		url.username !== "" ||
		url.password !== "" ||
		url.hash !== ""
	)
		throw invalidAuthorizationUrl(
			"Only the OpenAI OAuth authorization endpoint may be opened.",
		);
	return url.toString();
};

function registerAuthHandler({ ipcMain, shell, origin }) {
	ipcMain.handle("daiw-auth:open-authorization", async (event, payload) => {
		if (!isTrustedFrame(event, origin))
			throw Object.assign(
				new Error("The request did not come from the Studio main frame."),
				{ code: "UntrustedFrame" },
			);
		const url = parseAuthorizationUrl(payload?.url);
		await shell.openExternal(url, { activate: true });
		return { opened: true };
	});
}

module.exports = { parseAuthorizationUrl, registerAuthHandler };
