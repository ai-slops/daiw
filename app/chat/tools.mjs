import { createHash } from "node:crypto";
import { ServiceError } from "../errors.mjs";
import { toolSchemas } from "./schemas.mjs";

const MAX_RESULT = 64 * 1024;
const bounded = (value) => {
	const json = JSON.stringify(value);
	if (Buffer.byteLength(json) > MAX_RESULT) throw new ServiceError("ToolResultTooLarge", "The tool result exceeds the chat context limit.", 413);
	return value;
};

export class ChatTools {
	constructor({ store, projectService }) { this.store = store; this.projectService = projectService; }
	async execute(context, call) {
		if (call.namespace !== "DAIW") throw new ServiceError("ToolDenied", "This tool namespace is not available.", 403);
		const schema = toolSchemas[call.name];
		if (!schema) throw new ServiceError("ToolDenied", "This tool is not available.", 403);
		const argumentsValue = schema.parse(JSON.parse(call.arguments));
		const hash = createHash("sha256").update(JSON.stringify({ namespace: call.namespace, name: call.name, argumentsValue, projectId: context.projectId, revision: context.revision })).digest("hex");
		const prior = this.store.tool(context.turnId, call.callId);
		if (prior) {
			if (prior.call_hash !== hash) throw new ServiceError("ToolConflict", "The tool call ID was reused with different arguments.", 409);
			if (prior.status === "completed") return prior.result;
			throw new ServiceError("ToolBusy", "The prior tool call has not completed.", 409);
		}
		if (context.signal.aborted) throw context.signal.reason;
		this.store.startTool(context.turnId, call.callId, call.name, hash);
		try {
			const result = await this.#invoke({ ...context, callId: call.callId }, call.name, argumentsValue);
			bounded(result.output);
			this.store.finishTool(context.turnId, call.callId, result);
			return result;
		} catch (error) {
			this.store.failTool(context.turnId, call.callId, { code: error instanceof ServiceError ? error.code : "ToolFailed", message: error instanceof Error ? error.message : "The tool failed." });
			throw error;
		}
	}
	async #invoke(context, name, args) {
		switch (name) {
			case "project_context": {
				const response = await this.#request(context, "project.get", {});
				const project = response.state;
				return { output: { id: project.id, name: project.name, revision: project.revision, tempo: project.tempo, meter: project.meter, tracks: project.tracks.map(({ id, name, role, instrument }) => ({ id, name, role, instrument })), definitions: project.definitions.map(({ id, name, length, revision }) => ({ id, name, length, revision })), instances: project.instances.map(({ id, name, definitionId, trackId, start, length }) => ({ id, name, definitionId, trackId, start, length })) }, revision: project.revision };
			}
			case "project_query_notes": return this.#rpc(context, "project.queryNotes", args);
			case "project_explain": return this.#rpc(context, "project.explain", args);
			case "change_dry_run": return this.#rpc(context, "change.dryRun", { expectedRevision: context.revision, command: args.command });
			case "proposal_create": return this.#rpc(context, "proposal.create", { expectedRevision: context.revision, ...args });
			default: throw new ServiceError("ToolDenied", "This tool is not available.", 403);
		}
	}
	async #rpc(context, method, params) {
		const response = await this.#request(context, method, params);
		return { output: response.result, revision: response.state.revision };
	}
	async #request(context, method, params) {
		const response = await this.projectService.execute({ projectId: context.projectId, method, params, expectedRevision: context.revision, idempotencyKey: `chat:${context.turnId}:${context.callId ?? method}` }, "agent");
		if (!response.ok) throw new ServiceError(response.error.code, response.error.message, 422);
		return response;
	}
}
