import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { ChatService } from "../../app/chat/service.mjs";
import { createHandler } from "../../app/http.mjs";
import { MediaStore } from "../../app/media.mjs";
import { ProjectService } from "../../app/service.mjs";
import { createSignedProviderFixture } from "./signed-chat-provider-fixture.mjs";

export const startChatFlowFixture = async () => {
	const root = await mkdtemp(join(tmpdir(), "daiw-chat-flow-"));
	const projectService = new ProjectService({
		directory: join(root, "projects"),
		binary: resolve(process.env.DAIW_CORE_BINARY ?? "target/debug/daiw-core"),
	});
	const secondary = await projectService.execute({
		method: "project.create",
		params: { name: "QA Secondary", fixture: true },
	});
	const primary = await projectService.execute({
		method: "project.create",
		params: { name: "QA Primary", fixture: true },
	});
	if (!primary.ok || !secondary.ok)
		throw new TypeError("QA projects could not be created.");
	const providerFixture = await createSignedProviderFixture();
	providerFixture.instanceId = primary.state.instances[0]?.id;
	const chat = new ChatService({
		dataDirectory: join(root, "chat"),
		provider: providerFixture.provider,
		projectService,
	});
	const media = new MediaStore({
		directory: join(root, "projects"),
		service: projectService,
		binary: resolve(
			process.env.DAIW_RENDER_BINARY ?? "target/debug/daiw-render",
		),
	});
	await media.initialize();
	const server = createServer(
		createHandler({
			service: projectService,
			media,
			staticRoot: resolve(process.env.DAIW_STATIC_ROOT ?? "dist"),
			chat,
		}),
	);
	server.listen(0, "127.0.0.1");
	await once(server, "listening");
	const address = server.address();
	if (address === null || typeof address === "string")
		throw new TypeError("QA server did not expose a TCP address.");
	return {
		baseUrl: `http://127.0.0.1:${address.port}`,
		primaryProjectId: primary.state.id,
		secondaryProjectId: secondary.state.id,
		debugSnapshot() {
			return {
				providerRequests: providerFixture.requests,
				turns: chat.store.db
					.prepare("SELECT id,status,assistant_text,error FROM turns")
					.all(),
				events: chat.store.db
					.prepare(
						"SELECT turn_id,sequence,name,data FROM events ORDER BY sequence",
					)
					.all(),
			};
		},
		prepareLogin(loginId, code, identity) {
			const row = chat.store.db
				.prepare("SELECT data FROM pending_logins WHERE id=?")
				.get(loginId);
			if (row === undefined)
				throw new TypeError("Pending login was not stored.");
			providerFixture.prepare(code, JSON.parse(row.data).nonce, identity);
		},
		async completeAutoLogin(login, code, identity) {
			this.prepareLogin(login.loginId, code, identity);
			const callback = new URL(login.redirectUri);
			callback.searchParams.set("code", code);
			callback.searchParams.set(
				"state",
				new URL(login.authorizationUrl).searchParams.get("state"),
			);
			callback.searchParams.set("client_id", "oaiapp_qa_fixture");
			try {
				const response = await fetch(callback);
				await response.text();
			} catch (error) {
				if (!(error instanceof TypeError)) throw error;
			}
		},
		async close() {
			server.closeAllConnections();
			await new Promise((resolveClose) => server.close(resolveClose));
			await chat.close();
			await projectService.close();
			await rm(root, { recursive: true, force: true });
		},
	};
};
