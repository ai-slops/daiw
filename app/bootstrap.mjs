import { existsSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { join } from 'node:path';

export async function bootstrap(root) {
  const run = (command, args) => new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: root, stdio: 'inherit', shell: process.platform === 'win32' });
    child.on('error', reject);
    child.on('exit', (code) => code === 0 ? resolve() : reject(new Error(`${command} exited with ${code}`)));
  });
  if (!existsSync(join(root, 'node_modules/zod/package.json'))) await run('npm', ['ci', '--ignore-scripts']);
  const suffix = process.platform === 'win32' ? '.exe' : '';
  if (!process.env.DAIW_CORE_BINARY && !existsSync(join(root, `target/debug/daiw-core${suffix}`))) {
    if (process.platform === 'win32') await run('cargo', ['build']);
    else await run('sh', ['scripts/rust.sh', 'build']);
  }
  if (!existsSync(join(root, 'dist/index.html'))) await run('npm', ['run', 'build']);
}
