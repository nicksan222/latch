#!/usr/bin/env node

import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, mkdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const project = resolve(here, '../../..');
const defaults = {
  output: join(project, '.github/assets/demo.gif'),
  width: 1280,
  fps: 10,
  keepContainer: false,
  help: false
};

const usage = `Usage: bun run demo:record -- [options]

Build and record a real native Latch session in an isolated Docker/Xvfb container.

Options:
  --output PATH       GIF destination (default: .github/assets/demo.gif)
  --width PIXELS      Output width, 640-1920 (default: 1280)
  --fps RATE          Capture/output rate, 1-30 (default: 10)
  --image NAME        Container image (default: latch-bun-dev)
  --binary PATH       Optional compatible host binary (skips the desktop build)
  --keep-container    Keep the disposable container for investigation
  -h, --help          Show this help

The default path builds .devcontainer/Dockerfile when needed, installs with Bun,
and builds the seeder and desktop inside Docker. Named caches are reused between runs.
Environment overrides: LATCH_DEMO_IMAGE, LATCH_DEMO_BINARY, CONTAINER_ENGINE.`;

function integer(value, name, min, max) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < min || parsed > max)
    throw new Error(`${name} must be between ${min} and ${max}`);
  return parsed;
}

export function parseArgs(argv) {
  const result = { ...defaults };
  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    if (option === '-h' || option === '--help') result.help = true;
    else if (option === '--keep-container') result.keepContainer = true;
    else if (['--output', '--width', '--fps', '--image', '--binary'].includes(option)) {
      const value = argv[++index];
      if (!value) throw new Error(`${option} requires a value`);
      if (option === '--output') result.output = value;
      if (option === '--width') result.width = integer(value, 'width', 640, 1920);
      if (option === '--fps') result.fps = integer(value, 'fps', 1, 30);
      if (option === '--image') result.image = value;
      if (option === '--binary') result.binary = value;
    } else throw new Error(`unknown option: ${option}`);
  }
  return result;
}

export function buildContainerArgs({ image, name }) {
  return [
    'create',
    '--init',
    '--name',
    name,
    '--user',
    'vscode',
    '-e',
    'HOME=/home/vscode',
    '-e',
    'DISPLAY=:99',
    '-e',
    'CARGO_HOME=/home/vscode/.cargo',
    '-e',
    'LATCH_CONFIG_DIR=/workspaces/latch/.dev/latch/config',
    '-e',
    'LATCH_DATA_DIR=/workspaces/latch/.dev/latch/data',
    '-e',
    'LATCH_RUNTIME_DIR=/workspaces/latch/.dev/latch/runtime',
    '-e',
    'LATCH_RAM_DIR=/workspaces/latch/.dev/latch/ram',
    '-e',
    'CARGO_TARGET_DIR=/home/vscode/.cache/latch-targets/demo-recording',
    '-v',
    'latch-demo-cargo:/home/vscode/.cargo',
    '-v',
    'latch-demo-target:/home/vscode/.cache/latch-targets',
    '-v',
    'latch-demo-bun:/home/vscode/.bun/install/cache',
    '--workdir',
    '/workspaces/latch',
    image,
    'tail',
    '-f',
    '/dev/null'
  ];
}

export function buildContainerInitArgs() {
  return [
    'chown',
    '-R',
    'vscode:vscode',
    '/home/vscode/.cargo',
    '/home/vscode/.cache/latch-targets',
    '/home/vscode/.bun/install/cache',
    '/workspaces/latch',
    '/output'
  ];
}

export function buildCommands() {
  return [
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
  ];
}

function command(name, args, options = {}) {
  const result = spawnSync(name, args, { stdio: 'inherit', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${name} exited with status ${result.status}`);
}

function succeeds(name, args) {
  return spawnSync(name, args, { stdio: 'ignore' }).status === 0;
}
function executable(name) {
  try {
    return execFileSync('sh', ['-lc', `command -v ${name}`], { encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
}

function snapshotCheckout(destination) {
  const excluded = new Set(['.git', '.dev', 'node_modules', 'target']);
  cpSync(project, destination, {
    recursive: true,
    filter(source) {
      const path = relative(project, source);
      return path === '' || !excluded.has(path.split(sep)[0]);
    }
  });
}

function dockerExec(engine, container, args, { root = false } = {}) {
  command(engine, ['exec', '--user', root ? 'root' : 'vscode', container, ...args]);
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`demo:record: ${error.message}\n\n${usage}`);
    process.exitCode = 2;
    return;
  }
  if (options.help) {
    console.log(usage);
    return;
  }

  const engine = process.env.CONTAINER_ENGINE || executable('docker') || executable('podman');
  if (!engine) throw new Error('Docker or Podman is required');
  const selectedImage = options.image || process.env.LATCH_DEMO_IMAGE;
  const image = selectedImage || 'latch-bun-dev';
  const binaryOption = options.binary || process.env.LATCH_DEMO_BINARY;
  const binary = binaryOption ? resolve(binaryOption) : null;
  if (binary && !statSync(binary).isFile()) throw new Error(`desktop binary not found: ${binary}`);
  if (selectedImage && !succeeds(engine, ['image', 'inspect', image])) {
    throw new Error(`container image not found: ${image}`);
  }
  if (!selectedImage) {
    console.log(`Building ${image} from .devcontainer/Dockerfile`);
    command(engine, [
      'build',
      '--tag',
      image,
      '--file',
      join(project, '.devcontainer/Dockerfile'),
      project
    ]);
  }

  const name = `latch-demo-${process.pid}`;
  const staging = mkdtempSync(join(tmpdir(), 'latch-demo-source-'));
  const snapshot = join(staging, 'latch');
  const output = isAbsolute(options.output)
    ? options.output
    : resolve(process.cwd(), options.output);
  mkdirSync(dirname(output), { recursive: true });
  let created = false;
  try {
    console.log('Creating a disposable checkout snapshot');
    snapshotCheckout(snapshot);
    command(engine, buildContainerArgs({ image, name }), { stdio: 'ignore' });
    created = true;
    command(engine, ['start', name], { stdio: 'ignore' });
    dockerExec(engine, name, ['mkdir', '-p', '/workspaces/latch', '/output'], { root: true });
    command(engine, ['cp', `${snapshot}/.`, `${name}:/workspaces/latch`]);
    dockerExec(engine, name, buildContainerInitArgs(), { root: true });

    let desktop = '/home/vscode/.cache/latch-targets/demo-recording/debug/latch-desktop';
    let seed = '/home/vscode/.cache/latch-targets/demo-recording/debug/examples/seed_dev';
    if (binary) {
      const siblingSeed = join(dirname(binary), 'examples/seed_dev');
      if (!succeeds('test', ['-f', siblingSeed]))
        throw new Error(`accelerated binary requires its matching seed executable: ${siblingSeed}`);
      command(engine, ['cp', binary, `${name}:/opt/latch-desktop`]);
      command(engine, ['cp', siblingSeed, `${name}:/opt/seed_dev`]);
      desktop = '/opt/latch-desktop';
      seed = '/opt/seed_dev';
    } else {
      for (const buildCommand of buildCommands()) dockerExec(engine, name, buildCommand);
    }
    dockerExec(engine, name, [
      'env',
      `LATCH_DEMO_BINARY=${desktop}`,
      `LATCH_DEMO_SEED=${seed}`,
      `LATCH_DEMO_FPS=${options.fps}`,
      `LATCH_DEMO_WIDTH=${options.width}`,
      'dbus-run-session',
      '--',
      'xvfb-run',
      '-n',
      '99',
      '-s',
      '-screen 0 1920x1200x24',
      'python3',
      '/workspaces/latch/tooling/development/demo-recording/record_ui.py'
    ]);
    command(engine, ['cp', `${name}:/output/demo.gif`, output]);
    console.log(`Wrote ${output}`);
  } finally {
    rmSync(staging, { recursive: true, force: true });
    if (created && options.keepContainer) console.log(`Kept container ${name}`);
    else if (created) command(engine, ['rm', '--force', name], { stdio: 'ignore' });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
