import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

// Build first, then own the actual server process so signals reach its graceful
// shutdown handler instead of stopping only an intermediate `cargo run` process.
let child = spawn(
  'cargo',
  ['build', '--locked', '--no-default-features', '-p', 'latch-web-bridge', '--message-format=json'],
  {
    cwd: new URL('../..', import.meta.url),
    stdio: ['ignore', 'pipe', 'inherit']
  }
);
let executable;
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
const lines = createInterface({ input: child.stdout });
lines.on('line', (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }
  if (message.reason === 'compiler-artifact' && message.target.name === 'latch-web-bridge')
    executable = message.executable;
  if (message.reason === 'compiler-message' && message.message.rendered)
    process.stderr.write(message.message.rendered);
});
child.on('error', (error) => {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
});
child.on('exit', (code, signal) => {
  if (code !== 0 || signal || !executable) {
    process.exitCode = code || 1;
    return;
  }
  child = spawn(executable, process.argv.slice(2), { stdio: 'inherit' });
  child.on('error', (error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
  child.on('exit', (code) => {
    process.exitCode = code ?? 1;
  });
});
