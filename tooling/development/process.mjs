import { spawn } from 'node:child_process';
import { once } from 'node:events';

export function spawnOwned(command, args, options) {
  return spawn(command, args, { ...options, detached: true });
}
export async function stopOwned(child, gracefulSignal = 'SIGTERM') {
  if (!child?.pid) return;
  const signal = (name) => {
    try {
      process.kill(-child.pid, name);
      return true;
    } catch (error) {
      if (error.code === 'ESRCH') return false;
      throw error;
    }
  };
  const exited =
    child.exitCode === null && !child.signalCode ? once(child, 'exit') : Promise.resolve();
  if (!signal(gracefulSignal)) return;
  const timeout = setTimeout(() => signal('SIGKILL'), 10_000);
  timeout.unref();
  try {
    await exited;
  } finally {
    // A task runner can exit before Vite; the group belongs solely to this command.
    signal('SIGKILL');
    clearTimeout(timeout);
  }
}
