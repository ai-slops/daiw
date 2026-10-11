import { toResponseInputItems } from "openai/lib/responses/ResponseInputItems.mjs";
import { ServiceError } from "../errors.mjs";
import { providerTools } from "./schemas.mjs";

const MAX_BYTES = 256 * 1024;
const MAX_EVENTS = 2048;
const MAX_HISTORY_ITEMS = 128;
const terminalStatuses = new Set(["completed", "failed", "cancelled", "interrupted"]);
const safeProviderCode = (value) => typeof value === "string" && /^[a-z0-9_]{1,80}$/i.test(value) ? value : "ProviderFailed";

export class ChatRunner {
	#runs = new Map();
	constructor({ store, auth, provider, tools, onEvent }) { this.store = store; this.auth = auth; this.provider = provider; this.tools = tools; this.onEvent = onEvent; }
	start(context) {
		const controller = new AbortController();
		const timeout = setTimeout(() => controller.abort(new ServiceError("TurnTimeout", "The turn exceeded its 120 second deadline.", 408)), 120_000);
		const run = { controller, owner: context.owner, accountId: context.accountId, eventCount: 0 };
		const promise = this.#run(context, run).catch((error) => this.#fail(context.turnId, error)).finally(() => { clearTimeout(timeout); this.#runs.delete(context.turnId); });
		run.promise = promise;
		this.#runs.set(context.turnId, run);
	}
	cancel(turnId) { this.#runs.get(turnId)?.controller.abort(new ServiceError("TurnCancelled", "The turn was cancelled.", 499)); }
	abortAccount(owner, accountId) { for (const [turnId, run] of this.#runs) if (run.owner === owner && run.accountId === accountId) this.cancel(turnId); }
	async close() { const runs = [...this.#runs.values()]; for (const run of runs) run.controller.abort(new ServiceError("HostShutdown", "The host is shutting down.", 503)); await Promise.allSettled(runs.map((run) => run.promise)); }
	async #run(context, run) {
		const signal = run.controller.signal;
		const credentials = await this.auth.credentials(context.owner, context.accountId);
		const groups = [...this.store.historyGroups(context.sessionId), [{ role: "user", content: context.text }]];
		while (groups.length > 1 && (groups.flat().length > MAX_HISTORY_ITEMS || Buffer.byteLength(JSON.stringify(groups.flat())) > MAX_BYTES)) groups.shift();
		let input = groups.flat();
		let revision = context.revision;
		let assistantBytes = 0;
		let toolBytes = 0;
		let toolCalls = 0;
		for (let iteration = 0; iteration < 6; iteration += 1) {
			if (signal.aborted) throw signal.reason;
			const stream = await this.provider.responses({ model: context.model, input, tools: providerTools, store: false, stream: true }, credentials.accessToken, signal);
			let completedResponse;
			const completedItems = [];
			const calls = [];
			for await (const event of stream) {
				if (signal.aborted) throw signal.reason;
				if (event.type === "response.output_text.delta") {
					assistantBytes += Buffer.byteLength(event.delta);
					if (assistantBytes > MAX_BYTES) throw new ServiceError("OutputLimit", "The model output exceeded the display limit.", 413);
					this.#event(context.turnId, "text.delta", { text: event.delta });
					this.store.appendAssistantText(context.turnId, event.delta);
				}
				if (event.type === "response.output_item.done") {
					completedItems.push(event.item);
					if (event.item?.type === "function_call") calls.push({ callId: event.item.call_id, namespace: event.item.namespace, name: event.item.name, arguments: event.item.arguments });
				}
				if (event.type === "response.failed") throw new ServiceError(safeProviderCode(event.response?.error?.code), "The model response failed.", 502);
				if (event.type === "response.incomplete") throw new ServiceError("ProviderIncomplete", "The model response was incomplete.", 502);
				if (event.type === "response.completed") completedResponse = event.response;
			}
			if (!completedResponse) throw new ServiceError("ProviderInterrupted", "The model stream ended before completion.", 502);
			let output;
			try { output = toResponseInputItems(completedItems.length > 0 ? completedItems : completedResponse.output ?? []); }
			catch { throw new ServiceError("ProviderInvalidOutput", "The model returned invalid output items.", 502); }
			if (Buffer.byteLength(JSON.stringify(output)) > MAX_BYTES) throw new ServiceError("OutputLimit", "The model output exceeded the history limit.", 413);
			this.store.appendHistory(context.turnId, output);
			if (calls.length === 0) { const text = this.store.turn(context.turnId).assistant_text; this.#terminal(context.turnId, "completed", text); return; }
			if (toolCalls + calls.length > 8) throw new ServiceError("ToolLimit", "The turn exceeded the tool call limit.", 422);
			const outputs = [];
			for (const call of calls) {
				toolCalls += 1;
				this.#event(context.turnId, "tool.started", { callId: call.callId, name: call.name });
				const result = await this.tools.execute({ ...context, revision, signal }, call);
				revision = result.revision;
				const encoded = JSON.stringify(result.output);
				toolBytes += Buffer.byteLength(encoded);
				if (toolBytes > MAX_BYTES) throw new ServiceError("ToolResultLimit", "The turn exceeded the aggregate tool result limit.", 413);
				this.#event(context.turnId, "tool.completed", { callId: call.callId, name: call.name, result: result.output });
				const outputItem = { type: "function_call_output", call_id: call.callId, output: encoded };
				outputs.push(outputItem);
				this.store.appendHistory(context.turnId, [outputItem]);
			}
			input = [...input, ...output, ...outputs];
			if (input.length > MAX_HISTORY_ITEMS || Buffer.byteLength(JSON.stringify(input)) > MAX_BYTES) throw new ServiceError("HistoryLimit", "The completed tool context exceeds the history limit.", 413);
		}
		throw new ServiceError("IterationLimit", "The turn exceeded the model iteration limit.", 422);
	}
	#fail(turnId, error) {
		const prior = this.store.turn(turnId);
		if (terminalStatuses.has(prior.status)) return;
		const code = error instanceof ServiceError ? error.code : "ServiceFailure";
		const status = code === "TurnCancelled" ? "cancelled" : code === "HostShutdown" ? "interrupted" : "failed";
		const normalized = { code, message: error instanceof ServiceError ? error.message : "The turn failed." };
		this.#terminal(turnId, status, prior.assistant_text, normalized);
	}
	#terminal(turnId, status, assistantText, error) {
		const run = this.#runs.get(turnId);
		if (run.eventCount >= MAX_EVENTS) throw new ServiceError("EventLimit", "The turn exceeded the event limit.", 413);
		run.eventCount += 1;
		const data = { turnId, ...(error ? { error } : {}) };
		const name = `turn.${status}`;
		const sequence = this.store.finishTurn(turnId, status, assistantText, error, name, data);
		this.onEvent(turnId, { sequence, name, data });
	}
	#event(turnId, name, data) {
		const run = this.#runs.get(turnId);
		if (run.eventCount >= MAX_EVENTS - 1) throw new ServiceError("EventLimit", "The turn exceeded the event limit.", 413);
		run.eventCount += 1;
		const sequence = this.store.appendEvent(turnId, name, data);
		this.onEvent(turnId, { sequence, name, data });
	}
}
