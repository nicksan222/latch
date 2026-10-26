import { once } from 'node:events';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnOwned, stopOwned } from '../development/process.mjs';

const require = createRequire(import.meta.url);
export async function runPlaywright(args, cli = require.resolve('@playwright/test/cli')) {
  // Preserve monochrome requests without exporting conflicting Node controls.
  const env = { ...process.env };
  if (env.NO_COLOR !== undefined) {
    env.FORCE_COLOR = '0';
    delete env.NO_COLOR;
  }
  // Own the controller directly: a task runner can exit before controller teardown ends.
  const child = spawnOwned(process.execPath, [cli, 'test', ...args], {
    cwd: new URL('.', import.meta.url),
    env,
    stdio: 'inherit'
  });
  let stopping = false;
  const shutdown = () => {
    if (stopping) return;
    stopping = true;
    // Playwright handles SIGINT by awaiting worker and web-server teardown.
    void stopOwned(child, 'SIGINT');
  };
  const signals = ['SIGINT', 'SIGTERM'];
  for (const signal of signals) process.on(signal, shutdown);
  try {
    const [code] = await once(child, 'exit');
    return code ?? 1;
  } finally {
    for (const signal of signals) process.off(signal, shutdown);
    await stopOwned(child);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    process.exitCode = await runPlaywright(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
