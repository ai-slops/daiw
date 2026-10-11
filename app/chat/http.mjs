import { z } from "zod";
import { ServiceError } from "../errors.mjs";
import { accountSchema, completeSchema, loginSchema, sessionSchema, turnSchema } from "./schemas.mjs";
import { streamEvents } from "./sse.mjs";
const cookieName = "daiw_chat_sid";
const parseCookies = (header = "") => Object.fromEntries(header.split(";").map((part) => part.trim().split("=", 2)).filter(([key, value]) => key && value).map(([key, value]) => [key, decodeURIComponent(value)]));

async function jsonBody(request) {
	let size = 0;
	const chunks = [];
	for await (const chunk of request) {
		size += chunk.length;
		if (size > 32 * 1024) throw new ServiceError("ResourceLimit", "Chat request exceeds the size limit.", 413);
		chunks.push(chunk);
	}
	return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
}

function send(response, status, value, headers = {}) {
	response.writeHead(status, { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...headers });
	response.end(JSON.stringify(value));
}

export function createChatRoute(chat) {
	return async ({ request, response, url, authority, expectedOrigin, isHttps }) => {
		if (!url.pathname.startsWith("/api/chat/")) return false;
		try {
			if (request.method === "POST") {
				const origin = request.headers.origin;
				if (typeof origin !== "string" || new URL(origin).origin !== expectedOrigin || new URL(origin).host !== authority) throw new ServiceError("PermissionDenied", "Origin does not match this application.", 403);
			}
			const sid = parseCookies(request.headers.cookie)[cookieName];
			let owner = chat.owner(sid);
			const hostname = new URL(expectedOrigin).hostname;
			const callbackMode = new Set(["127.0.0.1", "localhost", "[::1]", "::1"]).has(hostname) ? "auto" : "paste";
			if (request.method === "GET" && url.pathname === "/api/chat/auth/status") {
				let setCookie;
				if (!owner) { const created = chat.createOwner(); owner = created.owner; setCookie = `${cookieName}=${encodeURIComponent(created.sid)}; Path=/; HttpOnly; SameSite=Strict;${isHttps ? " Secure;" : ""} Max-Age=31536000`; }
				return send(response, 200, chat.status(owner, callbackMode), setCookie ? { "set-cookie": setCookie } : {});
			}
			if (!owner) throw new ServiceError("Unauthorized", "A chat browser session is required.", 401);
			if (request.method === "POST" && url.pathname === "/api/chat/auth/login") return send(response, 200, await chat.login(owner, loginSchema.parse(await jsonBody(request)), callbackMode));
			if (request.method === "POST" && url.pathname === "/api/chat/auth/login/complete") return send(response, 200, await chat.complete(owner, completeSchema.parse(await jsonBody(request))));
			const loginStatus = /^\/api\/chat\/auth\/login\/([^/]+)$/.exec(url.pathname);
			if (request.method === "GET" && loginStatus) return send(response, 200, chat.loginStatus(owner, z.uuid().parse(decodeURIComponent(loginStatus[1]))));
			if (request.method === "POST" && url.pathname === "/api/chat/auth/logout") { const input = accountSchema.parse(await jsonBody(request)); return send(response, 200, await chat.logout(owner, input.accountId)); }
			if (request.method === "POST" && url.pathname === "/api/chat/auth/select") { const input = accountSchema.parse(await jsonBody(request)); return send(response, 200, chat.select(owner, input.accountId)); }
			if (request.method === "GET" && url.pathname === "/api/chat/models") return send(response, 200, await chat.models(owner, z.uuid().parse(url.searchParams.get("accountId"))));
			if (request.method === "GET" && url.pathname === "/api/chat/sessions") return send(response, 200, chat.sessions(owner, z.string().min(1).parse(url.searchParams.get("projectId")), z.uuid().parse(url.searchParams.get("accountId"))));
			if (request.method === "POST" && url.pathname === "/api/chat/sessions") return send(response, 200, await chat.createSession(owner, sessionSchema.parse(await jsonBody(request))));
			const events = /^\/api\/chat\/sessions\/([^/]+)\/turns\/([^/]+)\/events$/.exec(url.pathname);
			if (request.method === "GET" && events) return await streamEvents(chat, owner, decodeURIComponent(events[1]), decodeURIComponent(events[2]), request, response, url);
			const cancel = /^\/api\/chat\/sessions\/([^/]+)\/turns\/([^/]+)\/cancel$/.exec(url.pathname);
			if (request.method === "POST" && cancel) return send(response, 200, chat.cancel(owner, decodeURIComponent(cancel[1]), decodeURIComponent(cancel[2])));
			const turns = /^\/api\/chat\/sessions\/([^/]+)\/turns$/.exec(url.pathname);
			if (request.method === "POST" && turns) return send(response, 202, chat.startTurn(owner, decodeURIComponent(turns[1]), turnSchema.parse(await jsonBody(request))));
			const session = /^\/api\/chat\/sessions\/([^/]+)$/.exec(url.pathname);
			if (request.method === "GET" && session) return send(response, 200, chat.session(owner, decodeURIComponent(session[1])));
			throw new ServiceError("NotFound", "Chat API method not found.", 404);
		} catch (error) {
			if (error instanceof ServiceError) return send(response, error.status, { error: { code: error.code, message: error.message } });
			if (error instanceof z.ZodError || error instanceof SyntaxError || error instanceof TypeError) return send(response, 400, { error: { code: "InvalidRequest", message: error.message } });
			return send(response, 500, { error: { code: "ServiceFailure", message: "The chat operation failed." } });
		}
	};
}
