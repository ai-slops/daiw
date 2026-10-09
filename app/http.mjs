import { randomBytes, timingSafeEqual } from "node:crypto";
import { z } from "zod";
import { ServiceError } from "./errors.mjs";
import { createRenderRoute } from "./render-http.mjs";
import { rpcSchema } from "./service.mjs";
import { serveStatic } from "./static-http.mjs";

const safeEqual = (a, b) =>
	typeof a === "string" &&
	Buffer.byteLength(a) === Buffer.byteLength(b) &&
	timingSafeEqual(Buffer.from(a), Buffer.from(b));
const domainStatus = (code) =>
	({
		StaleRevision: 409,
		SelectionStale: 409,
		IdempotencyConflict: 409,
		EmptyHistory: 409,
		PermissionDenied: 403,
		MissingProject: 404,
		MissingAsset: 404,
		ResourceLimit: 413,
		SelectionLimitExceeded: 413,
	})[code] ?? 422;

async function body(request, limit = 2 * 1024 * 1024) {
	let length = 0;
	const chunks = [];
	for await (const chunk of request) {
		length += chunk.length;
		if (length > limit)
			throw new ServiceError(
				"ResourceLimit",
				"Request exceeds the size limit.",
				413,
			);
		chunks.push(chunk);
	}
	return Buffer.concat(chunks);
}

export function createHandler({
	service,
	media,
	staticRoot,
	agentToken,
	allowedHosts = new Set(),
}) {
	const session = randomBytes(32).toString("hex");
	const send = (response, status, value) => {
		response.writeHead(status, {
			"content-type": "application/json; charset=utf-8",
			"cache-control": "no-store",
		});
		response.end(JSON.stringify(value));
	};
	const serveRender = createRenderRoute({ service, media, send });
	return async (request, response) => {
		response.setHeader("X-Content-Type-Options", "nosniff");
		response.setHeader("Referrer-Policy", "same-origin");
		response.setHeader(
			"Content-Security-Policy",
			"default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self'; font-src 'self' data:; frame-ancestors 'self'",
		);
		try {
			const url = new URL(request.url ?? "/", "http://localhost");
			const localHosts = new Set([
				`127.0.0.1:${request.socket.localPort}`,
				`localhost:${request.socket.localPort}`,
				`[::1]:${request.socket.localPort}`,
				...allowedHosts,
			]);
			if (!localHosts.has(request.headers.host))
				throw new ServiceError(
					"PermissionDenied",
					"This hostname is not allowed to access DAIW.",
					403,
				);
			if (url.pathname.startsWith("/api/")) {
				if (request.headers["sec-fetch-site"] === "cross-site")
					throw new ServiceError(
						"PermissionDenied",
						"Cross-site access is not allowed.",
						403,
					);
				const origin = request.headers.origin;
				const trustedProxy = ["127.0.0.1", "::1", "::ffff:127.0.0.1"].includes(
					request.socket.remoteAddress,
				);
				const host = trustedProxy
					? (request.headers["x-forwarded-host"] ?? request.headers.host)
					: request.headers.host;
				if (
					!localHosts.has(host) ||
					(origin &&
						new URL(origin).host !== host &&
						!["localhost:5173", "127.0.0.1:5173"].includes(
							new URL(origin).host,
						))
				)
					throw new ServiceError(
						"PermissionDenied",
						"Origin does not match this application.",
						403,
					);
				if (request.method === "GET" && url.pathname === "/api/session")
					return send(response, 200, { token: session });
				if (request.method === "GET" && url.pathname === "/api/health")
					return send(response, 200, { status: "ok", protocolVersion: 1 });
				if (request.method === "GET" && url.pathname === "/api/projects")
					return send(response, 200, { projects: service.store.list() });
				if (request.method === "POST") {
					const isAgent = url.pathname === "/api/agent";
					const allowed = isAgent
						? agentToken &&
							safeEqual(request.headers.authorization, `Bearer ${agentToken}`)
						: safeEqual(request.headers["x-daiw-session"], session);
					if (!allowed)
						throw new ServiceError(
							"PermissionDenied",
							"A valid session or scoped agent token is required.",
							403,
						);
					if (url.pathname === "/api/rpc" || isAgent) {
						const command = rpcSchema.parse(
							JSON.parse((await body(request)).toString("utf8")),
						);
						const result = await service.execute(
							command,
							isAgent ? "agent" : "human",
						);
						const status = result.ok ? 200 : domainStatus(result.error.code);
						return send(response, status, result);
					}
					const match = /^\/api\/projects\/([^/]+)\/import-audio$/.exec(
						url.pathname,
					);
					if (match) {
						const data = await body(request, 21 * 1024 * 1024);
						const form = await new Request("http://localhost/upload", {
							method: "POST",
							headers: {
								"content-type": request.headers["content-type"] ?? "",
							},
							body: data,
						}).formData();
						const key = request.headers["x-daiw-idempotency"];
						const result = await media.importAudio(
							decodeURIComponent(match[1]),
							form,
							typeof key === "string" ? key : undefined,
						);
						return send(
							response,
							result.ok ? 200 : domainStatus(result.error.code),
							result,
						);
					}
				}
				const waveform =
					/^\/api\/projects\/([^/]+)\/assets\/([^/]+)\/peaks$/.exec(
						url.pathname,
					);
				if (request.method === "GET" && waveform)
					return send(
						response,
						200,
						await media.waveform(
							decodeURIComponent(waveform[1]),
							decodeURIComponent(waveform[2]),
						),
					);
				const resource =
					/^\/api\/projects\/([^/]+)\/(export|render\.wav|render-info)$/.exec(
						url.pathname,
					);
				if (request.method === "GET" && resource) {
					const projectId = decodeURIComponent(resource[1]);
					if (resource[2] === "export") {
						response.setHeader(
							"Content-Disposition",
							'attachment; filename="project.daiw.json"',
						);
						const exported = await service.execute({
							projectId,
							method: "project.export",
							params: {},
						});
						return send(
							response,
							exported.ok ? 200 : domainStatus(exported.error.code),
							exported.ok ? exported.result : exported,
						);
					}
					return await serveRender({
						request,
						response,
						url,
						projectId,
						kind: resource[2],
					});
				}
				throw new ServiceError("NotFound", "API method not found.", 404);
			}
			await serveStatic({ request, response, staticRoot, url });
		} catch (error) {
			if (response.headersSent || response.destroyed) {
				response.destroy();
				return;
			}
			if (error instanceof ServiceError)
				return send(response, error.status, {
					ok: false,
					error: { code: error.code, message: error.message },
				});
			if (
				error instanceof z.ZodError ||
				error instanceof SyntaxError ||
				error instanceof TypeError
			)
				return send(response, 400, {
					ok: false,
					error: { code: "InvalidRequest", message: error.message },
				});
			// Only the host boundary reports unexpected I/O failures; never log project payloads.
			process.stderr.write(
				`DAIW request failed: ${error instanceof Error ? error.message : "Unknown error"}\n`,
			);
			send(response, 500, {
				ok: false,
				error: {
					code: "ServiceFailure",
					message:
						"The operation failed. Project state was not replaced. Check service diagnostics.",
				},
			});
		}
	};
}
