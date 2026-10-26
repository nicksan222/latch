#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const [action, ...options] = process.argv.slice(2);
const desktop = options.length === 1 && options[0] === '--desktop';
const scope = desktop
  ? ['--package', 'latch-desktop']
  : ['--workspace', '--exclude', 'latch-desktop', '--no-default-features'];
// cargo fmt has its own argument parser: no --locked or feature selection.
const formatScope = desktop
  ? ['--package', 'latch-desktop']
  : [
      '--package',
      'latch-rpc',
      '--package',
      'latch-core',
      '--package',
      'latch-cli',
      '--package',
      'latch-web-bridge'
    ];
const commands = {
  format: ['fmt', ...formatScope],
  'format:check': ['fmt', ...formatScope, '--', '--check'],
  check: ['check', '--locked', ...scope, '--all-targets'],
  clippy: ['clippy', '--locked', ...scope, '--all-targets', '--', '-D', 'warnings'],
  // Public library documentation avoids the cross-package `latch` lib/bin output collision.
  docs: ['doc', '--locked', ...scope, '--no-deps', '--lib'],
  test: ['test', '--locked', ...scope]
};
const quality = ['format:check', 'check', 'clippy', 'docs'];
const steps = action === 'all' ? [...quality, 'test'] : action === 'quality' ? quality : [action];
if ((options.length > 0 && !desktop) || steps.some((step) => !Object.hasOwn(commands, step))) {
  console.error(
    'Usage: node tooling/rust/run.mjs <format|format:check|check|clippy|docs|test|quality|all> [--desktop]'
  );
  process.exit(2);
}
for (const step of steps) {
  const args = commands[step];
  console.log(`cargo ${args.join(' ')}`);
  const env = { ...process.env };
  if (step === 'docs') env.RUSTDOCFLAGS = `${env.RUSTDOCFLAGS ?? ''} -D warnings`.trim();
  const result = spawnSync('cargo', args, { cwd: root, env, stdio: 'inherit' });
  if (result.error) console.error(result.error.message);
  if (result.status !== 0) process.exit(result.status ?? 1);
}
