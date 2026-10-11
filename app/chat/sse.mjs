import { once } from "node:events";
import { z } from "zod";
import { ServiceError } from "../errors.mjs";

const terminal = new Set(["turn.completed", "turn.failed", "turn.cancelled", "turn.interrupted"]);

async function writeEvent(response, event) {
	const writable = response.write(`id: ${event.sequence}\nevent: ${event.name}\ndata: ${JSON.stringify(event.data)}\n\n`);
	if (!writable && !response.destroyed) await Promise.race([once(response, "drain"), once(response, "close")]);
}

export async function streamEvents(chat, owner, sessionId, turnId, request, response, url) {
	const session = chat.store.session(owner, sessionId);
	const turn = chat.store.turn(turnId);
	if (turn.session_id !== session.id) throw new ServiceError("MissingTurn", "Chat turn not found.", 404);
	const afterText = url.searchParams.get("after") ?? request.headers["last-event-id"] ?? "0";
	let last = z.coerce.number().int().nonnegative().parse(afterText);
	response.writeHead(200, { "content-type": "text/event-stream; charset=utf-8", "cache-control": "no-store", connection: "keep-alive" });
	let closed = false;
	let wake;
	const queued = [];
	const listener = (event) => { queued.push(event); wake?.(); };
	const close = () => { closed = true; wake?.(); };
	chat.events.on(turnId, listener);
	request.once("close", close);
	try {
		for (;;) {
			const page = chat.store.events(turnId, last, 256);
			for (const event of page) { if (event.sequence > last) { await writeEvent(response, event); last = event.sequence; } }
			if (page.length < 256) break;
		}
		for (;;) {
			while (queued.length) {
				const event = queued.shift();
				if (event.sequence <= last) continue;
				await writeEvent(response, event);
				last = event.sequence;
				if (terminal.has(event.name)) return response.end();
			}
			for (const event of chat.store.events(turnId, last, 256)) queued.push(event);
			if (queued.length) continue;
			if (closed || chat.store.turn(turnId).status !== "running") return response.end();
			await new Promise((resolve) => { wake = resolve; });
			wake = undefined;
		}
	} finally {
		chat.events.off(turnId, listener);
		request.off("close", close);
	}
}
