#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function nodeSupported(version) {
  const match = /^v?(\d+)\./.exec(version);
  return Boolean(match && Number(match[1]) >= 24);
}

export function toolchainRequirements(text) {
  const channel = /^\s*channel\s*=\s*"([^"]+)"/m.exec(text)?.[1];
  const components = /components\s*=\s*\[([^\]]*)\]/m.exec(text)?.[1];
  if (!channel || !components)
    throw new Error('rust-toolchain.toml must declare channel and components');
  return { channel, components: [...components.matchAll(/"([^"]+)"/g)].map((match) => match[1]) };
}

export function evaluate(snapshot, requirements) {
  const checks = [];
  const add = (name, ok, fix, optional = false) => checks.push({ name, ok, fix, optional });
  add('Node >=24', nodeSupported(snapshot.node), 'Install Node 24 or newer, then run npm ci.');
  for (const tool of ['npm', 'git', 'rustup', 'cargo', 'rustc']) {
    add(
      tool,
      Boolean(snapshot.tools[tool]),
      `Install ${tool}; use the devcontainer for the supported Linux environment.`
    );
  }
  add(
    `Rust ${requirements.channel}`,
    snapshot.rustVersion?.split(/\s+/)[1] === requirements.channel,
    'Run rustup show from the repository root to install the checked-in toolchain.'
  );
  for (const component of requirements.components) {
    add(
      `Rust component ${component}`,
      snapshot.components.some((line) => line === component || line.startsWith(`${component}-`)),
      `Run rustup component add ${component} from the repository root.`
    );
  }
  if (snapshot.platform === 'linux') {
    add(
      'Linux Tauri libraries',
      snapshot.desktopLibraries,
      'Use the devcontainer, or install pkg-config, libssl-dev, libwebkit2gtk-4.1-dev, libayatana-appindicator3-dev and librsvg2-dev.',
      true
    );
    add(
      'Linux desktop tools',
      snapshot.desktopTools,
      'Use the devcontainer for Xvfb, xauth, x11vnc, openbox, xdotool, dbus-run-session and websockify.',
      true
    );
    add(
      'noVNC ready on 127.0.0.1:5900',
      snapshot.novnc,
      'Start the devcontainer desktop with npm run dev:desktop; inspect npm run dev:desktop:status.',
      true
    );
  }
  return checks;
}

function run(command, args) {
  try {
    return execFileSync(command, args, {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
      timeout: 10000,
      env: { ...process.env, RUSTUP_AUTO_INSTALL: '0' }
    }).trim();
  } catch {
    return '';
  }
}

export async function main() {
  const requirements = toolchainRequirements(
    readFileSync(new URL('../../rust-toolchain.toml', import.meta.url), 'utf8')
  );
  const tools = Object.fromEntries(
    ['npm', 'git', 'rustup', 'cargo', 'rustc'].map((tool) => [tool, run(tool, ['--version'])])
  );
  let novnc = false;
  try {
    const response = await fetch('http://127.0.0.1:5900/vnc.html', {
      signal: AbortSignal.timeout(2000),
      redirect: 'error'
    });
    novnc = response.ok && (await response.text()).includes('noVNC');
  } catch {
    /* A stopped desktop is optional for web development. */
  }
  const snapshot = {
    node: process.version,
    platform: process.platform,
    tools,
    rustVersion: tools.rustc,
    components: run('rustup', ['component', 'list', '--installed']).split('\n'),
    desktopLibraries: Boolean(
      run('pkg-config', [
        '--modversion',
        'openssl',
        'webkit2gtk-4.1',
        'ayatana-appindicator3-0.1',
        'librsvg-2.0'
      ])
    ),
    desktopTools: [
      'Xvfb',
      'xauth',
      'x11vnc',
      'openbox',
      'xdotool',
      'dbus-run-session',
      'websockify'
    ].every((tool) => Boolean(run('sh', ['-c', 'command -v "$1"', 'doctor', tool]))),
    novnc
  };
  const strictDesktop = process.argv.includes('--desktop');
  const checks = evaluate(snapshot, requirements);
  for (const check of checks) {
    console.log(
      `${check.ok ? 'OK' : check.optional && !strictDesktop ? 'OPTIONAL' : 'ERROR'} ${check.name}`
    );
    if (!check.ok) console.log(`  ${check.fix}`);
  }
  if (checks.some((check) => !check.ok && (!check.optional || strictDesktop))) process.exitCode = 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
