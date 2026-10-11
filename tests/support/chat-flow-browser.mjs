import assert from "node:assert/strict";

export const authenticate = async (page, fixture, code, identity) => {
	const login = await page.evaluate(async () => {
		const response = await fetch("/api/chat/auth/login", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ mode: "auto" }),
		});
		return { ok: response.ok, body: await response.json() };
	});
	assert.equal(login.ok, true, JSON.stringify(login.body));
	assert.equal(login.body.completionMode, "auto");
	const authorizationUrl = new URL(login.body.authorizationUrl);
	assert.equal(authorizationUrl.origin, "https://auth.openai.com");
	assert.equal(authorizationUrl.pathname, "/api/accounts/authorize");
	assert.equal(authorizationUrl.username, "");
	assert.equal(authorizationUrl.password, "");
	assert.equal(authorizationUrl.port, "");
	assert.equal(authorizationUrl.hash, "");

	const pending = await page.evaluate(async (loginId) => {
		const response = await fetch(`/api/chat/auth/login/${loginId}`);
		return response.json();
	}, login.body.loginId);
	assert.deepEqual(pending, { status: "pending" });
	await fixture.completeAutoLogin(login.body, code, identity);
	const completion = await page.evaluate(async (loginId) => {
		for (let attempt = 0; attempt < 20; attempt += 1) {
			const response = await fetch(`/api/chat/auth/login/${loginId}`);
			const status = await response.json();
			if (status.status !== "pending") return status;
			await new Promise((resolve) => setTimeout(resolve, 25));
		}
		return { status: "timeout" };
	}, login.body.loginId);
	assert.equal(completion.status, "completed", JSON.stringify(completion));
	return {
		accountId: completion.activeAccountId,
		authorizationUrl: authorizationUrl.href,
		loginId: login.body.loginId,
	};
};

export const openAssistant = async (page) => {
	await page.getByRole("button", { name: "Assistant" }).click();
	await page.getByRole("heading", { name: "Assistant" }).waitFor();
};

export const openProject = async (page, name) => {
	await page.getByRole("button", { name: "Projects", exact: true }).click();
	const dialog = page.getByRole("dialog", { name: "Projects & history" });
	await dialog.getByRole("button", { name: new RegExp(name) }).click();
	await page.getByRole("button", { name: new RegExp(name) }).waitFor();
};

export const assertNoHorizontalOverflow = async (page) => {
	const result = await page.evaluate(() => ({
		viewport: window.innerWidth,
		documentWidth: document.documentElement.scrollWidth,
		offenders: [...document.querySelectorAll("body *")]
			.filter((element) => {
				const rect = element.getBoundingClientRect();
				return (
					rect.width > 0 &&
					(rect.left < -1 || rect.right > window.innerWidth + 1)
				);
			})
			.slice(0, 10)
			.map((element) => ({
				tag: element.tagName,
				className: element.className,
				label: element.getAttribute("aria-label"),
			})),
	}));
	assert.ok(
		result.documentWidth <= result.viewport,
		`Document overflowed ${result.documentWidth}px into ${result.viewport}px: ${JSON.stringify(result.offenders)}`,
	);
	return result;
};
