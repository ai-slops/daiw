import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';

// A release guard, not legal advice: fail on absent licenses and strong copyleft.
const forbidden = /(^|[ (])(AGPL|GPL)-/;
const dependencies = JSON.parse(execFileSync('npm', ['ls', '--all', '--json', '--long'], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }));
const issues = [];
const seen = new Set();
async function inspect(node) {
  if (node.path && !seen.has(node.path)) {
    seen.add(node.path);
    const manifest = JSON.parse(await readFile(`${node.path}/package.json`, 'utf8'));
    if (!manifest.private) {
      const license = typeof manifest.license === 'string' ? manifest.license : manifest.license?.type;
      if (!license || forbidden.test(license)) issues.push(`${manifest.name}: ${license ?? 'missing license'}`);
    }
  }
  for (const dependency of Object.values(node.dependencies ?? {})) await inspect(dependency);
}
await inspect(dependencies);
const rust = JSON.parse(process.env.DAIW_RUST_METADATA
  ? await readFile(process.env.DAIW_RUST_METADATA, 'utf8')
  : execFileSync('cargo', ['metadata', '--format-version', '1'], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }));
for (const dependency of rust.packages) {
  if (!dependency.license || forbidden.test(dependency.license)) issues.push(`${dependency.name}: ${dependency.license ?? 'missing license'}`);
}
if (issues.length > 0) {
  process.stderr.write(`Dependency license review required:\n${issues.join('\n')}\n`);
  process.exitCode = 1;
} else process.stdout.write(`License metadata checked for ${seen.size} JavaScript and ${rust.packages.length} Rust packages.\n`);
