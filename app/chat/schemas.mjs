import { z } from "zod";

const id = z.string().min(1).max(256);
const selector = z.discriminatedUnion("type", [
	z.strictObject({ type: z.literal("all") }),
	z.strictObject({ type: z.literal("stableIds"), noteIds: z.array(id).min(1).max(4096) }),
	z.strictObject({ type: z.literal("namedSlot"), slot: id }),
	z.strictObject({ type: z.literal("beatRange"), start: z.number().nonnegative(), end: z.number().positive() }),
	z.strictObject({ type: z.literal("pitchRange"), min: z.number(), max: z.number() }),
]);

export const loginSchema = z.strictObject({ accountId: z.uuid().optional(), mode: z.enum(["auto", "paste"]).optional() });
export const completeSchema = z.strictObject({ loginId: z.uuid(), callbackUrl: z.url().max(4096) });
export const accountSchema = z.strictObject({ accountId: z.uuid() });
export const sessionSchema = z.strictObject({ projectId: id, accountId: z.uuid(), model: id });
export const turnSchema = z.strictObject({ clientTurnId: z.uuid(), text: z.string().min(1).max(16 * 1024).refine((value) => Buffer.byteLength(value) <= 16 * 1024) });

export const toolSchemas = Object.freeze({
	project_context: z.strictObject({}),
	project_query_notes: z.strictObject({ instanceId: id, scope: z.enum(["source", "instance"]), selector, maxResults: z.number().int().min(1).max(4096) }),
	project_explain: z.strictObject({ instanceId: id }),
	change_dry_run: z.strictObject({ command: z.record(z.string(), z.unknown()) }),
	proposal_create: z.strictObject({ instanceId: id, request: z.string().min(1).max(4096), operations: z.array(z.record(z.string(), z.unknown())).max(64).optional() }),
});

const objectSchema = (properties, required = Object.keys(properties)) => ({ type: "object", properties, required, additionalProperties: false });
const string = { type: "string", minLength: 1 };
export const providerTools = [{
	type: "namespace", name: "DAIW", description: "Read the selected DAIW project, preview typed changes, and create proposals for human review.",
	tools: [
		{ type: "function", name: "project_context", strict: true, parameters: objectSchema({}) },
		{ type: "function", name: "project_query_notes", strict: false, parameters: objectSchema({ instanceId: string, scope: { type: "string", enum: ["source", "instance"] }, selector: { type: "object" }, maxResults: { type: "integer", minimum: 1, maximum: 4096 } }) },
		{ type: "function", name: "project_explain", strict: true, parameters: objectSchema({ instanceId: string }) },
		{ type: "function", name: "change_dry_run", strict: false, parameters: objectSchema({ command: { type: "object" } }) },
		{ type: "function", name: "proposal_create", strict: false, parameters: objectSchema({ instanceId: string, request: string, operations: { type: "array", maxItems: 64, items: { type: "object" } } }, ["instanceId", "request"]) },
	],
}];
