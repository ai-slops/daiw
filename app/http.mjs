import { randomBytes, timingSafeEqual } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join, extname } from 'node:path';
import { z } from 'zod';
import { ServiceError } from './errors.mjs';
import { rpcSchema } from './service.mjs';
import { containedPath } from './paths.mjs';

const contentTypes = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2', '.json': 'application/json' };
const safeEqual = (a, b) => typeof a === 'string' && Buffer.byteLength(a) === Buffer.byteLength(b) && timingSafeEqual(Buffer.from(a), Buffer.from(b));
const domainStatus = (code) => ({ StaleRevision: 409, SelectionStale: 409, IdempotencyConflict: 409, EmptyHistory: 409, PermissionDenied: 403, MissingProject: 404, MissingAsset: 404, ResourceLimit: 413, SelectionLimitExceeded: 413 })[code] ?? 422;

async function body(request, limit = 2 * 1024 * 1024) {
  let length = 0;
  const chunks = [];
  for await (const chunk of request) {
    length += chunk.length;
    if (length > limit) throw new ServiceError('ResourceLimit', 'Request exceeds the size limit.', 413);
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

export function createHandler({ service, media, staticRoot, agentToken, allowedHosts = new Set() }) {
  const session = randomBytes(32).toString('hex');
  const send = (response, status, value) => {
    response.writeHead(status, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' });
    response.end(JSON.stringify(value));
  };
  return async (request, response) => {
    response.setHeader('X-Content-Type-Options', 'nosniff');
    response.setHeader('Referrer-Policy', 'same-origin');
    response.setHeader('Content-Security-Policy', "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self'; font-src 'self' data:; frame-ancestors 'self'");
    try {
      const url = new URL(request.url ?? '/', 'http://localhost');
      const localHosts = new Set([`127.0.0.1:${request.socket.localPort}`, `localhost:${request.socket.localPort}`, `[::1]:${request.socket.localPort}`, ...allowedHosts]);
      if (!localHosts.has(request.headers.host)) throw new ServiceError('PermissionDenied', 'This hostname is not allowed to access DAIW.', 403);
      if (url.pathname.startsWith('/api/')) {
        if (request.headers['sec-fetch-site'] === 'cross-site') throw new ServiceError('PermissionDenied', 'Cross-site access is not allowed.', 403);
        const origin = request.headers.origin;
        const trustedProxy = ['127.0.0.1', '::1', '::ffff:127.0.0.1'].includes(request.socket.remoteAddress);
        const host = trustedProxy ? request.headers['x-forwarded-host'] ?? request.headers.host : request.headers.host;
        if (!localHosts.has(host) || (origin && new URL(origin).host !== host && !['localhost:5173', '127.0.0.1:5173'].includes(new URL(origin).host))) throw new ServiceError('PermissionDenied', 'Origin does not match this application.', 403);
        if (request.method === 'GET' && url.pathname === '/api/session') return send(response, 200, { token: session });
        if (request.method === 'GET' && url.pathname === '/api/health') return send(response, 200, { status: 'ok', protocolVersion: 1 });
        if (request.method === 'GET' && url.pathname === '/api/projects') return send(response, 200, { projects: service.store.list() });
        if (request.method === 'POST') {
          const isAgent = url.pathname === '/api/agent';
          const allowed = isAgent ? agentToken && safeEqual(request.headers.authorization, `Bearer ${agentToken}`) : safeEqual(request.headers['x-daiw-session'], session);
          if (!allowed) throw new ServiceError('PermissionDenied', 'A valid session or scoped agent token is required.', 403);
          if (url.pathname === '/api/rpc' || isAgent) {
            const command = rpcSchema.parse(JSON.parse((await body(request)).toString('utf8')));
            const result = await service.execute(command, isAgent ? 'agent' : 'human');
            const status = result.ok ? 200 : domainStatus(result.error.code);
            return send(response, status, result);
          }
          const match = /^\/api\/projects\/([^/]+)\/import-audio$/.exec(url.pathname);
          if (match) {
            const data = await body(request, 21 * 1024 * 1024);
            const form = await new Request('http://localhost/upload', { method: 'POST', headers: { 'content-type': request.headers['content-type'] ?? '' }, body: data }).formData();
            const key = request.headers['x-daiw-idempotency'];
            const result = await media.importAudio(decodeURIComponent(match[1]), form, typeof key === 'string' ? key : undefined);
            return send(response, result.ok ? 200 : domainStatus(result.error.code), result);
          }
        }
        const waveform = /^\/api\/projects\/([^/]+)\/assets\/([^/]+)\/peaks$/.exec(url.pathname);
        if (request.method === 'GET' && waveform) return send(response, 200, await media.waveform(decodeURIComponent(waveform[1]), decodeURIComponent(waveform[2])));
        const resource = /^\/api\/projects\/([^/]+)\/(export|render\.wav)$/.exec(url.pathname);
        if (request.method === 'GET' && resource) {
          const projectId = decodeURIComponent(resource[1]);
          if (resource[2] === 'export') {
            response.setHeader('Content-Disposition', 'attachment; filename="project.daiw.json"');
            const exported = await service.execute({ projectId, method: 'project.export', params: {} });
            return send(response, exported.ok ? 200 : domainStatus(exported.error.code), exported.ok ? exported.result : exported);
          }
          const rendered = await media.render(projectId, url.searchParams.get('proposalId'), url.searchParams.get('expectedRevision'));
          response.writeHead(200, { 'content-type': 'audio/wav', 'content-length': rendered.wav.length, 'X-DAIW-Revision': rendered.revision, ETag: `"${rendered.key}"`, 'cache-control': 'private, no-cache', 'Content-Disposition': 'inline; filename="daiw-mix.wav"' });
          return response.end(rendered.wav);
        }
        throw new ServiceError('NotFound', 'API method not found.', 404);
      }
      if (request.method !== 'GET' && request.method !== 'HEAD') throw new ServiceError('MethodNotAllowed', 'Method not allowed.', 405);
      const pathname = decodeURIComponent(url.pathname);
      const path = containedPath(staticRoot, pathname === '/' ? '/index.html' : pathname);
      let file;
      try { file = await readFile(path); } catch (error) {
        if (!(error instanceof Error && 'code' in error && error.code === 'ENOENT')) throw error;
        if (extname(path)) throw new ServiceError('NotFound', 'Resource not found.', 404);
        file = await readFile(join(staticRoot, 'index.html'));
      }
      response.writeHead(200, { 'content-type': contentTypes[extname(path)] ?? 'text/html; charset=utf-8', 'cache-control': pathname.includes('/assets/') ? 'public,max-age=31536000,immutable' : 'no-cache' });
      response.end(request.method === 'HEAD' ? undefined : file);
    } catch (error) {
      if (error instanceof ServiceError) return send(response, error.status, { ok: false, error: { code: error.code, message: error.message } });
      if (error instanceof z.ZodError || error instanceof SyntaxError || error instanceof TypeError) return send(response, 400, { ok: false, error: { code: 'InvalidRequest', message: error.message } });
      // Only the host boundary reports unexpected I/O failures; never log project payloads.
      process.stderr.write(`DAIW request failed: ${error instanceof Error ? error.message : 'Unknown error'}\n`);
      send(response, 500, { ok: false, error: { code: 'ServiceFailure', message: 'The operation failed. Project state was not replaced. Check service diagnostics.' } });
    }
  };
}
