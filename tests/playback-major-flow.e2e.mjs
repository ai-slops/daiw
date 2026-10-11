import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium, expect } from "@playwright/test";

const root = resolve(import.meta.dirname, "..");
const evidence = resolve(
	process.env.DAIW_EVIDENCE_DIR ?? ".artifacts/playback-major-flow/isolated",
);

const reservePort = async () => {
	const probe = createServer();
	await new Promise((resolveReady, reject) => {
		probe.once("error", reject);
		probe.listen(0, "127.0.0.1", resolveReady);
	});
	const address = probe.address();
	assert(address && typeof address === "object");
	await new Promise((resolveClosed, reject) =>
		probe.close((error) => (error ? reject(error) : resolveClosed())),
	);
	return address.port;
};

const childEnvironment = { ...process.env };
delete childEnvironment.DIM_DEVELOPMENT_URL_SOCKET;
let data, origin, service, serviceExit;
let serverOutput = "";

const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH;
let browser;
let context;
let page;
const browserErrors = [];
const renderResponses = [];
const qaName = `Playback QA ${Date.now()}`;

try {
	await mkdir(evidence, { recursive: true });
	data = await mkdtemp(join(tmpdir(), "daiw-playback-e2e-"));
	const port = await reservePort();
	origin = `http://127.0.0.1:${port}`;
	assert.equal(childEnvironment.DIM_DEVELOPMENT_URL_SOCKET, undefined);
	service = spawn(process.execPath, ["app/server.mjs"], {
		cwd: root,
		env: {
			...childEnvironment,
			DAIW_DATA_DIR: data,
			HOST: "127.0.0.1",
			PORT: String(port),
		},
		stdio: ["ignore", "pipe", "pipe"],
	});
	serviceExit = new Promise((resolveExit) => {
		service.once("error", resolveExit);
		service.once("exit", resolveExit);
	});
	await new Promise((resolveReady, rejectReady) => {
		service.once("error", rejectReady);
		service.once("exit", (code, signal) =>
			rejectReady(
				new Error(
					`Isolated DAIW server exited before readiness (${signal ?? code}).\n${serverOutput}`,
				),
			),
		);
		service.stdout.on("data", (chunk) => {
			serverOutput += chunk.toString();
			if (serverOutput.includes(`DAIW listening on port ${port}`)) resolveReady();
		});
		service.stderr.on("data", (chunk) => {
			serverOutput += chunk.toString();
		});
	});
	browser = await chromium.launch({
		headless: true,
		...(executablePath === undefined ? {} : { executablePath }),
		args: process.platform === "linux" ? ["--no-sandbox"] : [],
	});
	context = await browser.newContext({ viewport: { width: 1440, height: 960 } });
	page = await context.newPage();
	page.on("pageerror", (error) => browserErrors.push(error.message));
	page.on("console", (message) => {
		if (message.type() === "error") browserErrors.push(message.text());
	});
	page.on("response", (response) => {
		const url = response.url();
		if (!url.includes("/render-info") && !url.includes("/render.wav")) return;
		const headers = response.headers();
		renderResponses.push({
			url,
			status: response.status(),
			contentRange: headers["content-range"] ?? null,
			revision: headers["x-daiw-revision"] ?? null,
		});
	});
	await page.goto(origin);
	await page.locator("button.project-title").click();
	const dialog = page.getByRole("dialog");
	await dialog.getByRole("textbox", { name: "New project name", exact: true }).fill(qaName);
	await dialog.getByRole("button", { name: "Create project", exact: true }).click();
	await expect(page.getByRole("banner")).toContainText(qaName);
	await page.getByRole("button", { name: "Close projects", exact: true }).click();

	const position = page.getByLabel("Current musical position");
	const stoppedPosition = await position.textContent();
	await page.getByRole("button", { name: "Start playback", exact: true }).click();
	await expect(page.getByRole("button", { name: "Pause playback", exact: true })).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => position.textContent()).not.toBe(stoppedPosition);
	await page.screenshot({ path: join(evidence, "direct-play.png"), fullPage: true });

	await page.getByRole("button", { name: "Pause playback", exact: true }).click();
	await expect(page.getByRole("button", { name: "Start playback", exact: true })).toBeVisible();
	const pausedPosition = await position.textContent();
	await page.evaluate(
		() =>
			new Promise((resolveObservation) => {
				let remainingFrames = 12;
				const observe = () => {
					remainingFrames -= 1;
					if (remainingFrames === 0) resolveObservation(undefined);
					else requestAnimationFrame(observe);
				};
				requestAnimationFrame(observe);
			}),
	);
	await expect(position).toHaveText(pausedPosition ?? "001 / 01 / 000");
	await page.getByRole("button", { name: "Stop playback", exact: true }).click();
	await expect(position).toHaveText(stoppedPosition ?? "001 / 01 / 000");

	await page.getByRole("button", { name: "Start playback", exact: true }).click();
	await expect(page.getByRole("button", { name: "Pause playback", exact: true })).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => position.textContent()).not.toBe(stoppedPosition);
	await page.getByRole("button", { name: "Stop playback", exact: true }).click();
	await expect(position).toHaveText(stoppedPosition ?? "001 / 01 / 000");

	const grid = page.getByRole("button", { name: /notes\. Click or tap the grid/ });
	const notesBefore = await grid.getAttribute("aria-label");
	await grid.click({ position: { x: 40, y: 10 } });
	await expect(grid).not.toHaveAttribute("aria-label", notesBefore ?? "");
	await page.getByRole("button", { name: "Start playback", exact: true }).click();
	await expect(page.getByRole("button", { name: "Pause playback", exact: true })).toBeVisible({ timeout: 30_000 });
	await page.getByRole("button", { name: "Stop playback", exact: true }).click();
	await page.screenshot({ path: join(evidence, "reprepared-after-edit.png"), fullPage: true });

	const notesAfter = await grid.getAttribute("aria-label");
	await page.reload();
	await expect(page.getByRole("banner")).toContainText(qaName);
	await expect(page.getByRole("button", { name: notesAfter ?? "" })).toBeVisible();
	await page.getByRole("button", { name: "Start playback", exact: true }).click();
	await expect(page.getByRole("button", { name: "Pause playback", exact: true })).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => position.textContent()).not.toBe(stoppedPosition);
	await page.getByRole("button", { name: "Stop playback", exact: true }).click();

	const metadataResponses = renderResponses.filter(({ url }) =>
		url.includes("/render-info"),
	);
	const rangeResponses = renderResponses.filter(({ url }) =>
		url.includes("/render.wav"),
	);
	assert.equal(metadataResponses.some(({ status }) => status === 200), true);
	assert.equal(
		new Set(metadataResponses.map(({ revision }) => revision)).size >= 2,
		true,
		"Editing invalidates prepared audio and requests the new revision",
	);
	assert.equal(rangeResponses.length > 0, true, "Playback streams ranged PCM");
	assert.equal(
		rangeResponses.every(
			({ status, contentRange }) => status === 206 && contentRange !== null,
		),
		true,
		"Every observed PCM request is a successful byte range",
	);

	await page.reload();
	await expect(page.getByRole("banner")).toContainText(qaName);
	const latestMetadata = metadataResponses.at(-1);
	assert(latestMetadata?.revision);
	const latestMetadataResponse = await fetch(latestMetadata.url);
	assert.equal(latestMetadataResponse.status, 200);
	const latestMetadataBody = await latestMetadataResponse.json();
	assert.equal(
		typeof latestMetadataBody.pcmPeak === "number" && latestMetadataBody.pcmPeak > 0,
		true,
		"Prepared render metadata reports a nonzero PCM peak",
	);
	const projectId = new URL(latestMetadata.url).pathname.split("/")[3];
	assert(projectId);
	const session = await (await fetch(`${origin}/api/session`)).json();
	const externalEdit = await fetch(`${origin}/api/rpc`, {
		method: "POST",
		headers: {
			"content-type": "application/json",
			"X-DAIW-Session": session.token,
		},
		body: JSON.stringify({
			projectId,
			method: "change.apply",
			params: {
				expectedRevision: latestMetadata.revision,
				command: { type: "setTempo", tempo: 137 },
			},
		}),
	});
	assert.equal(externalEdit.status, 200);
	const staleRequestStart = renderResponses.length;
	await page.getByRole("button", { name: "Start playback", exact: true }).click();
	const preparationError = page.getByRole("alert");
	await expect(preparationError).toBeVisible();
	await expect
		.poll(() =>
			renderResponses
				.slice(staleRequestStart)
				.some(({ status, url }) => status === 409 && url.includes("/render-info")),
		)
		.toBe(true);
	const staleResponse = renderResponses
		.slice(staleRequestStart)
		.find(({ status, url }) => status === 409 && url.includes("/render-info"));
	assert(staleResponse);
	assert.equal(
		new URL(staleResponse.url).searchParams.get("expectedRevision"),
		latestMetadata.revision,
	);
	await expect(page.getByRole("button", { name: "Start playback", exact: true })).toBeVisible();
	await expect(page.getByRole("button", { name: "Pause playback", exact: true })).toHaveCount(0);
	await page.screenshot({ path: join(evidence, "stale-revision-error.png"), fullPage: true });
	await expect(preparationError).not.toContainText(
		"prepare must complete before playback",
	);

	await page.reload();
	await expect(page.getByRole("banner")).toContainText(qaName);
	const recoveryStart = await position.textContent();
	await page.getByRole("button", { name: "Start playback", exact: true }).click();
	await expect(page.getByRole("button", { name: "Pause playback", exact: true })).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => position.textContent()).not.toBe(recoveryStart);
	await page.getByRole("button", { name: "Stop playback", exact: true }).click();
	await page.screenshot({ path: join(evidence, "recovered-playback.png"), fullPage: true });

	assert.equal(browserErrors.length, 1);
	assert.match(browserErrors[0], /409 \(Conflict\)$/);
	assert.equal(await page.getByRole("alert").count(), 0);
	process.stdout.write(
		`Playback major flow passed at ${origin}: direct play, pause, stop, replay, revision reprepare, reload persistence, surfaced stale-revision error, and recovery. Evidence: ${evidence}\n`,
	);
} finally {
	try {
		await context?.close();
		await browser?.close();
	} finally {
		if (service?.pid !== undefined && service.exitCode === null && service.signalCode === null) {
			service.kill("SIGTERM");
		}
		await serviceExit;
		if (data !== undefined) await rm(data, { recursive: true, force: true });
		const runtimeEvidence = JSON.stringify({ origin, qaName, browserErrors, renderResponses }, null, 2);
		await writeFile(join(evidence, "runtime-evidence.json"), runtimeEvidence);
	}
}
