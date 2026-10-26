import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { withBridge } from './with-bridge.mjs';

export function frontendArguments(input) {
  if (input.length && (input.length !== 2 || input[0] !== '--port'))
    throw new Error('Unsupported frontend arguments');
  const port = input.length ? Number(input[1]) : 1422;
  if (!Number.isInteger(port) || port < 1024 || port > 65535)
    throw new Error('Invalid frontend port');
  return { port, args: ['-C', 'apps/desktop', 'dev', '--port', String(port)] };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { port, args } = frontendArguments(process.argv.slice(2));
  process.exitCode = await withBridge(
    'npm',
    ['exec', 'vp', '--', ...args],
    [`http://127.0.0.1:${port}`, `http://localhost:${port}`]
  );
}
