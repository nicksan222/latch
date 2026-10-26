import { once } from 'node:events';
import { createServer } from 'node:net';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';
import { spawnOwned, stopOwned } from './process.mjs';

const root = new URL('../..', import.meta.url);
async function freePort() {
  const server = createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const port = server.address().port;
  await new Promise((done, reject) => server.close((error) => (error ? reject(error) : done())));
  return port;
}
/** Own the dev-only Rust server for exactly one command; never reuse another run. */
export async function withBridge(command, args, origins = []) {
  const port = await freePort();
  const url = `http://127.0.0.1:${port}`;
  const env = { ...process.env, LATCH_WEB_BRIDGE_URL: url };
  // Some parent terminals export both; Node warns whenever both are present.
  if (env.FORCE_COLOR !== undefined) delete env.NO_COLOR;
  const bridge = spawnOwned(
    process.execPath,
    [
      'tooling/web-bridge/run.mjs',
      '--port',
      String(port),
      ...origins.flatMap((origin) => ['--origin', origin])
    ],
    { cwd: root, env, stdio: 'inherit' }
  );
  let child;
  let stopping = false;
  const shutdown = () => {
    stopping = true;
    void stopOwned(child);
    void stopOwned(bridge);
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
  let launchError;
  bridge.on('error', (error) => {
    launchError = error;
  });
  try {
    const deadline = Date.now() + 300_000;
    while (true) {
      if (launchError) throw launchError;
      if (stopping || bridge.exitCode !== null || bridge.signalCode)
        throw new Error('Rust fixture server stopped before startup');
      try {
        const response = await fetch(`${url}/__latch/health`, {
          signal: AbortSignal.timeout(1000)
        });
        if (response.status === 204) break;
      } catch {
        /* The owned server is still compiling or binding. */
      }
      if (Date.now() > deadline) throw new Error('Rust fixture server startup timed out');
      await delay(150);
    }
    child = spawnOwned(command, args, { cwd: root, env, stdio: 'inherit' });
    const onBridgeExit = () => {
      if (!stopping) {
        stopping = true;
        void stopOwned(child);
      }
    };
    bridge.on('exit', onBridgeExit);
    const [code] = await once(child, 'exit');
    bridge.off('exit', onBridgeExit);
    return stopping ? 1 : (code ?? 1);
  } finally {
    process.off('SIGINT', shutdown);
    process.off('SIGTERM', shutdown);
    await stopOwned(child);
    await stopOwned(bridge);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command, ...args] = process.argv.slice(2);
  if (!command) throw new Error('Command required');
  process.exitCode = await withBridge(command, args);
}
