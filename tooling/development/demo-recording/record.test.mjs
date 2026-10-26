import assert from 'node:assert/strict';
import test from 'node:test';

import { buildCommands, buildContainerArgs, buildContainerInitArgs, parseArgs } from './record.mjs';

test('parseArgs accepts the documented output and capture sizing options', () => {
  assert.deepEqual(
    parseArgs(['--output', '/tmp/demo.gif', '--width', '1280', '--fps', '10', '--keep-container']),
    { output: '/tmp/demo.gif', width: 1280, fps: 10, keepContainer: true, help: false }
  );
});

test('container initialization gives the unprivileged builder ownership of named caches', () => {
  assert.deepEqual(buildContainerInitArgs(), [
    'chown',
    '-R',
    'vscode:vscode',
    '/home/vscode/.cargo',
    '/home/vscode/.cache/latch-targets',
    '/home/vscode/.bun/install/cache',
    '/workspaces/latch',
    '/output'
  ]);
});

test('parseArgs rejects unsafe or malformed values', () => {
  assert.throws(() => parseArgs(['--width', '0']), /width must be between 640 and 1920/);
  assert.throws(() => parseArgs(['--fps', 'fast']), /fps must be between 1 and 30/);
  assert.throws(() => parseArgs(['--unknown']), /unknown option/);
});

test('default build uses Tauri build mode so the native binary embeds the frontend', () => {
  assert.deepEqual(buildCommands(), [
    ['bun', 'install', '--frozen-lockfile'],
    [
      'cargo',
      'build',
      '--locked',
      '-p',
      'latch-core',
      '--no-default-features',
      '--example',
      'seed_dev'
    ],
    ['bun', 'run', '--filter', '@latch/desktop', 'tauri', '--', 'build', '--debug', '--no-bundle']
  ]);
});

test('container args isolate state in a disposable container with reusable caches', () => {
  const args = buildContainerArgs({
    image: 'latch-quality-final',
    name: 'latch-demo-test'
  });

  assert.deepEqual(args.slice(0, 3), ['create', '--init', '--name']);
  assert.ok(args.includes('latch-demo-test'));
  assert.ok(args.includes('CARGO_HOME=/home/vscode/.cargo'));
  assert.ok(args.includes('latch-demo-cargo:/home/vscode/.cargo'));
  assert.ok(args.includes('latch-demo-target:/home/vscode/.cache/latch-targets'));
  assert.ok(args.includes('latch-demo-bun:/home/vscode/.bun/install/cache'));
  assert.ok(args.includes('LATCH_CONFIG_DIR=/workspaces/latch/.dev/latch/config'));
  assert.ok(args.includes('LATCH_DATA_DIR=/workspaces/latch/.dev/latch/data'));
  assert.ok(args.includes('LATCH_RUNTIME_DIR=/workspaces/latch/.dev/latch/runtime'));
  assert.ok(args.includes('LATCH_RAM_DIR=/workspaces/latch/.dev/latch/ram'));
  assert.ok(!args.some((arg) => arg.includes('/home/nicksan222')));
  assert.ok(!args.includes('--mount'));
  assert.ok(!args.includes('--volume'));
  assert.ok(!args.includes('-p'));
  assert.ok(!args.some((arg) => arg.includes('5900')));
});
