import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { ServiceError } from "./errors.mjs";
import { containedPath } from "./paths.mjs";

const contentTypes = {
	".html": "text/html; charset=utf-8",
	".js": "text/javascript; charset=utf-8",
	".css": "text/css; charset=utf-8",
	".svg": "image/svg+xml",
	".png": "image/png",
	".woff2": "font/woff2",
	".json": "application/json",
};

export async function serveStatic({ request, response, staticRoot, url }) {
	if (request.method !== "GET" && request.method !== "HEAD") {
		throw new ServiceError("MethodNotAllowed", "Method not allowed.", 405);
	}
	const pathname = decodeURIComponent(url.pathname);
	const path = containedPath(
		staticRoot,
		pathname === "/" ? "/index.html" : pathname,
	);
	let file;
	try {
		file = await readFile(path);
	} catch (error) {
		if (
			!(
				error instanceof Error &&
				"code" in error &&
				error.code === "ENOENT"
			)
		) {
			throw error;
		}
		if (extname(path)) {
			throw new ServiceError("NotFound", "Resource not found.", 404);
		}
		file = await readFile(join(staticRoot, "index.html"));
	}
	response.writeHead(200, {
		"content-type": contentTypes[extname(path)] ?? "text/html; charset=utf-8",
		"cache-control": pathname.includes("/assets/")
			? "public,max-age=31536000,immutable"
			: "no-cache",
	});
	response.end(request.method === "HEAD" ? undefined : file);
}
