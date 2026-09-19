import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { bootstrap } from './bootstrap.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
await bootstrap(root);
const [{ ProjectService }, { MediaStore }, { createHandler }] = await Promise.all([import('./service.mjs'), import('./media.mjs'), import('./http.mjs')]);
const suffix = process.platform === 'win32' ? '.exe' : '';
const directory = process.env.DAIW_DATA_DIR ?? resolve(root, 'data');
const service = new ProjectService({ directory, binary: process.env.DAIW_CORE_BINARY ?? resolve(root, `target/debug/daiw-core${suffix}`) });
const media = new MediaStore({ directory, service, binary: process.env.DAIW_RENDER_BINARY ?? resolve(root, `target/debug/daiw-render${suffix}`) });
await media.initialize();
const allowedHosts = new Set((process.env.DAIW_ALLOWED_ORIGINS ?? '').split(',').filter(Boolean).map((origin) => new URL(origin).host));
const server = createServer(createHandler({ service, media, staticRoot: resolve(root, 'dist'), agentToken: process.env.DAIW_AGENT_TOKEN, allowedHosts }));
server.requestTimeout = 150000;
server.headersTimeout = 10000;
server.listen(Number(process.env.PORT ?? 3000), process.env.HOST ?? '127.0.0.1', () => {
  process.stdout.write(`DAIW listening on port ${process.env.PORT ?? 3000}\n`);
  if (process.env.DIM_DEVELOPMENT_URL_SOCKET) {
    const exposure = spawn('dim-development-service', ['expose', '--name', 'daiw', '--port', process.env.PORT ?? '3000', '--ingress', 'https-ts', '--require-scheme', 'https'], { stdio: ['ignore', 'pipe', 'inherit'] });
    let registration = '';
    exposure.stdout.on('data', (chunk) => { registration += chunk; });
    exposure.on('close', (code) => {
      if (code === 0) {
        try { const url = new URL(registration.trim()); allowedHosts.add(url.host); process.stdout.write(`${url.href}\n`); }
        catch (error) { process.stderr.write(`DAIW preview URL was invalid: ${error instanceof Error ? error.message : 'Unknown error'}\n`); }
      }
    });
    exposure.on('error', (error) => process.stderr.write(`DAIW preview registration failed: ${error.message}\n`));
  }
});
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => server.close(() => { service.close().then(() => process.exit(0), () => process.exit(1)); }));
}
