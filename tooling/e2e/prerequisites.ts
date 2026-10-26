import { randomUUID } from 'node:crypto';

/** Keep trace cleanup local to this invocation, even when another thread runs tests. */
export function selectE2ERun(env: Record<string, string | undefined> = process.env) {
  const id = env.LATCH_E2E_RUN_ID ?? randomUUID();
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,80}$/.test(id)) throw new Error('Invalid E2E run ID');
  const port = Number(env.LATCH_E2E_PORT ?? 1423);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('Invalid E2E port');
  // Playwright workers inherit the controller's identity, rather than inventing their own.
  env.LATCH_E2E_RUN_ID = id;
  return { id, port };
}
