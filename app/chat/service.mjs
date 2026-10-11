import { randomBytes } from "node:crypto";
import { EventEmitter } from "node:events";
import { homedir } from "node:os";
import { join } from "node:path";
import { ServiceError } from "../errors.mjs";
import { ChatAuth } from "./auth.mjs";
import { OpenAIProvider } from "./provider.mjs";
import { ChatRunner } from "./runner.mjs";
import { ChatStore } from "./store.mjs";
import { ChatTools } from "./tools.mjs";

export class ChatService {
	constructor({ dataDirectory, provider = new OpenAIProvider(), projectService }) {
		this.store = new ChatStore(dataDirectory);
		this.provider = provider;
		this.projectService = projectService;
		this.auth = new ChatAuth({ store: this.store, provider });
		this.events = new EventEmitter();
		this.runner = new ChatRunner({ store: this.store, auth: this.auth, provider, tools: new ChatTools({ store: this.store, projectService }), onEvent: (turnId, event) => this.events.emit(turnId, event) });
	}
	createOwner() { const sid = randomBytes(32).toString("base64url"); return { sid, owner: this.store.owner(sid, true) }; }
	owner(sid, create = false) { return typeof sid === "string" ? this.store.owner(sid, create) : undefined; }
	status(owner, callbackMode) { return this.auth.status(owner, callbackMode); }
	loginStatus(owner, loginId) { return this.auth.loginStatus(owner, loginId); }
	login(owner, input, callbackMode) { return this.auth.login(owner, input, callbackMode); }
	complete(owner, input) { return this.auth.complete(owner, input.loginId, input.callbackUrl); }
	async logout(owner, accountId) { this.runner.abortAccount(owner, accountId); return this.auth.logout(owner, accountId); }
	select(owner, accountId) { this.store.select(owner, accountId); return { activeAccountId: accountId }; }
	async models(owner, accountId) { const credentials = await this.auth.credentials(owner, accountId); const models = await this.provider.models(credentials.accessToken); return { models: models.filter((model) => model.visibility === "list").map((model) => ({ id: model.slug, displayName: model.display_name })) }; }
	async createSession(owner, input) { this.store.account(owner, input.accountId); const catalog = await this.models(owner, input.accountId); if (!catalog.models.some((model) => model.id === input.model)) throw new ServiceError("InvalidModel", "The selected model is not available to this account.", 400); this.projectService.store.get(input.projectId); return { session: this.store.createSession(owner, input) }; }
	sessions(owner, projectId, accountId) { this.store.account(owner, accountId); return { sessions: this.store.sessions(owner, projectId, accountId) }; }
	session(owner, id) { const session = this.store.session(owner, id); return { session, turns: this.store.turns(id) }; }
	startTurn(owner, sessionId, input) {
		const session = this.store.session(owner, sessionId);
		const project = this.projectService.store.get(session.projectId);
		const turn = this.store.createTurn(sessionId, input.clientTurnId, input.text, project.revision);
		if (!turn.replay) this.runner.start({ owner, sessionId, turnId: turn.id, accountId: session.accountId, projectId: session.projectId, model: session.model, text: input.text, revision: project.revision });
		return { turnId: turn.id, eventsUrl: `/api/chat/sessions/${sessionId}/turns/${turn.id}/events` };
	}
	cancel(owner, sessionId, turnId) { this.store.session(owner, sessionId); const turn = this.store.turn(turnId); if (turn.session_id !== sessionId) throw new ServiceError("MissingTurn", "Chat turn not found.", 404); this.runner.cancel(turnId); return {}; }
	async close() { await this.runner.close(); this.auth.close(); this.store.close(); }
}

export function defaultChatDirectory() {
	const home = process.env.XDG_CONFIG_HOME ?? join(homedir(), ".config");
	return join(home, "daiw", "assistant");
}
