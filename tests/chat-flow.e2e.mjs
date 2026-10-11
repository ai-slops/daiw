import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright-core";
import {
	assertNoHorizontalOverflow,
	authenticate,
	openAssistant,
	openProject,
} from "./support/chat-flow-browser.mjs";
import { startChatFlowFixture } from "./support/chat-flow-fixture.mjs";

const evidenceDirectory = resolve(
	process.env.QA_EVIDENCE_DIR ?? ".artifacts/chat-flow",
);
const executablePath =
	process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ?? chromium.executablePath();

await mkdir(evidenceDirectory, { recursive: true });
const fixture = await startChatFlowFixture();
let browser;
let page;
const consoleErrors = [];
const pageErrors = [];
const failedResponses = [];
const streamRequests = [];
try {
	browser = await chromium.launch({
		executablePath,
		headless: true,
		args: ["--no-sandbox", "--disable-dev-shm-usage"],
	});
	const context = await browser.newContext({
		viewport: { width: 1440, height: 960 },
	});
	page = await context.newPage();
	page.on("console", (message) => {
		if (message.type() === "error") consoleErrors.push(message.text());
	});
	page.on("pageerror", (error) => pageErrors.push(error.message));
	page.on("response", (response) => {
		if (response.status() >= 400)
			failedResponses.push(`${response.status()} ${response.url()}`);
	});
	page.on("request", (request) => {
		const url = new URL(request.url());
		if (url.pathname.endsWith("/events")) streamRequests.push(url.href);
	});
	await page.goto(fixture.baseUrl, { waitUntil: "networkidle" });
	await page.getByRole("button", { name: /QA Primary/ }).waitFor();
	await openAssistant(page);
	await page.getByRole("button", { name: "Continue with ChatGPT" }).waitFor();

	const firstLogin = await authenticate(page, fixture, "qa-code-one", {
		subject: "qa-subject-one",
		email: "qa-one@example.com",
	});
	await page.reload({ waitUntil: "networkidle" });
	await openAssistant(page);
	await page.getByText("qa-one@example.com", { exact: true }).first().waitFor();
	await page.getByLabel("Model").selectOption("gpt-qa");
	await page.getByRole("button", { name: "New chat", exact: true }).click();
	await page.getByRole("checkbox").check();
	const composer = page.getByLabel("Message the project assistant");
	await composer.fill("Exercise every tool contract and create a proposal");
	await page.getByRole("button", { name: "Send message" }).click();
	await page
		.getByText("All project tools completed for human review.")
		.waitFor();
	const toolLabels = [
		"project context",
		"project query notes",
		"project explain",
		"change dry run",
		"proposal create",
	];
	for (const label of toolLabels)
		await page.getByText(label, { exact: true }).waitFor();

	await page.getByRole("tab", { name: "Proposal review" }).click();
	await page.getByText("Raise the selected phrase for review").waitFor();
	await page.getByRole("tab", { name: "Agent chat" }).click();
	await page.reload({ waitUntil: "networkidle" });
	await openAssistant(page);
	await page.locator(".chat-session-list button").first().click();
	await page
		.getByText("All project tools completed for human review.")
		.waitFor();
	assert.equal(
		await page
			.getByText("All project tools completed for human review.")
			.count(),
		1,
	);
	for (const label of toolLabels)
		assert.equal(await page.getByText(label, { exact: true }).count(), 1);

	await page.getByRole("checkbox").check();
	await page.getByRole("button", { name: "New chat", exact: true }).click();
	await composer.fill("Wait for cancellation");
	await page.getByRole("button", { name: "Send message" }).click();
	await page.getByText("Working…").waitFor();
	await Promise.all([
		page.waitForRequest((request) => {
			const url = new URL(request.url());
			return (
				url.pathname.endsWith("/events") &&
				url.searchParams.get("after") === "1"
			);
		}),
		(async () => {
			await page.reload({ waitUntil: "domcontentloaded" });
			await openAssistant(page);
			await page.locator(".chat-session-list button").first().click();
		})(),
	]);
	await page.getByText("Working…").waitFor();
	assert.equal(await page.getByText("Working…").count(), 1);
	await page.getByRole("button", { name: "Cancel response" }).click();
	await page.getByRole("button", { name: "Send message" }).waitFor();
	await page.getByRole("button", { name: "New chat", exact: true }).click();
	await page.getByRole("checkbox").check();
	await composer.fill("Trigger safe terminal failure");
	await page.getByRole("button", { name: "Send message" }).click();
	const terminalAlert = page
		.getByRole("alert")
		.filter({ hasText: "usage_limit" });
	await terminalAlert.waitFor();
	assert.match(await terminalAlert.innerText(), /The model response failed\./);
	assert.equal(
		await page.getByRole("button", { name: "Try again" }).isEnabled(),
		true,
	);
	await page.screenshot({
		path: resolve(evidenceDirectory, "terminal-error-1440.png"),
		fullPage: false,
	});
	await page.setViewportSize({ width: 375, height: 844 });
	await terminalAlert.scrollIntoViewIfNeeded();
	await page.screenshot({
		path: resolve(evidenceDirectory, "terminal-error-375.png"),
		fullPage: false,
	});
	await page.setViewportSize({ width: 1440, height: 960 });

	const secondLogin = await authenticate(page, fixture, "qa-code-two", {
		subject: "qa-subject-two",
		email: "qa-two@example.com",
	});
	assert.notEqual(secondLogin.accountId, firstLogin.accountId);
	await page.reload({ waitUntil: "networkidle" });
	await openAssistant(page);
	await page
		.getByRole("combobox", { name: "Account" })
		.selectOption({ label: "qa-one@example.com" });
	await page.getByText("qa-one@example.com", { exact: true }).first().waitFor();
	assert.equal(await page.getByRole("checkbox").isChecked(), false);

	await openProject(page, "QA Secondary");
	await openAssistant(page);
	await page.getByText("No saved sessions for this project.").waitFor();
	assert.equal(await page.getByRole("checkbox").isChecked(), false);
	await openProject(page, "QA Primary");
	await openAssistant(page);
	await page.locator(".chat-session-list button").last().click();
	await page
		.getByText("All project tools completed for human review.")
		.waitFor();

	await page.screenshot({
		path: resolve(evidenceDirectory, "assistant-1440.png"),
		fullPage: false,
	});
	const desktopOverflow = await assertNoHorizontalOverflow(page);
	await page.setViewportSize({ width: 375, height: 844 });
	await page.waitForTimeout(200);
	await page.getByRole("checkbox").check();
	await composer.fill("Keyboard focus check");
	const sendButton = page.getByRole("button", { name: "Send message" });
	await sendButton.waitFor();
	assert.equal(await sendButton.isEnabled(), true);
	await composer.focus();
	await page.keyboard.press("Tab");
	assert.equal(
		await sendButton.evaluate((element) => element === document.activeElement),
		true,
	);
	const undersizedTouchTargets = await page.evaluate(() =>
		[
			...document.querySelectorAll(
				".assistant-workspace button, .assistant-workspace a, .assistant-chat-panel button, .assistant-chat-panel a",
			),
		]
			.map((element) => ({
				label:
					element.getAttribute("aria-label") ?? element.textContent?.trim(),
				height: element.getBoundingClientRect().height,
			}))
			.filter(({ height }) => height > 0 && height < 44),
	);
	assert.deepEqual(undersizedTouchTargets, []);
	const mobileOverflow = await assertNoHorizontalOverflow(page);
	await page.screenshot({
		path: resolve(evidenceDirectory, "assistant-375.png"),
		fullPage: false,
	});

	assert.deepEqual(consoleErrors, []);
	assert.deepEqual(pageErrors, []);
	assert.deepEqual(failedResponses, []);
	await writeFile(
		resolve(evidenceDirectory, "results.json"),
		JSON.stringify(
			{
				baseUrl: fixture.baseUrl,
				viewports: [
					{ width: 1440, height: 960, overflow: desktopOverflow },
					{ width: 375, height: 844, overflow: mobileOverflow },
				],
				consoleErrors,
				pageErrors,
				failedResponses,
				streamRequests,
				auth: [firstLogin, secondLogin],
				toolLabels,
				undersizedTouchTargets,
				flows: [
					"unauthenticated login shell",
					"signed OAuth automatic loopback completion and per-login polling",
					"model and new session",
					"all five streamed project tools and proposal review",
					"numeric-cursor restoration and cancellation",
					"safe inline terminal failure alert",
					"completed history reload without duplicate events",
					"account switch resets scoped consent and model data",
					"project switch resets scoped consent and sessions",
					"keyboard focus and responsive overflow",
				],
			},
			null,
			2,
		),
	);
} catch (error) {
	if (page !== undefined)
		await page.screenshot({
			path: resolve(evidenceDirectory, "failure.png"),
			fullPage: false,
		});
	await writeFile(
		resolve(evidenceDirectory, "failure.json"),
		JSON.stringify(
			{
				message: error instanceof Error ? error.message : String(error),
				bodyText:
					page === undefined ? null : await page.locator("body").innerText(),
				consoleErrors,
				pageErrors,
				failedResponses,
				backend: fixture.debugSnapshot(),
			},
			null,
			2,
		),
	);
	throw error;
} finally {
	await browser?.close();
	await fixture.close();
}
