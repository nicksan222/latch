import assert from 'node:assert/strict';
import { once } from 'node:events';
import { test } from 'node:test';
import { spawnOwned, stopOwned } from './process.mjs';

// A nested server is the same ownership problem as npm -> Vite. Its inherited
// pipe stays open after the immediate parent exits unless the group is stopped.
test('shutdown releases descendants even after their immediate parent exits', async () => {
  const script = `const {spawn}=require('node:child_process'); const child=spawn(process.execPath,['-e', 'setInterval(()=>{},1000)'],{stdio:['ignore','ignore','ignore']}); console.log(child.pid); process.exit(0);`;
  const child = spawnOwned(process.execPath, ['-e', script], {
    stdio: ['ignore', 'pipe', 'inherit']
  });
  const exited = once(child, 'exit');
  const [data] = await once(child.stdout, 'data');
  const descendant = Number(data.toString().trim());
  await exited;
  assert.doesNotThrow(() => process.kill(descendant, 0));
  const cleanup = () => {
    try {
      process.kill(descendant, 'SIGKILL');
    } catch (error) {
      if (error.code !== 'ESRCH') throw error;
    }
  };
  try {
    await stopOwned(child);
    // Linux reports zombies until PID 1 reaps them, so inspect state as well.
    const { readFileSync } = await import('node:fs');
    let running = false;
    try {
      running = !readFileSync(`/proc/${descendant}/stat`, 'utf8').split(') ')[1].startsWith('Z');
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
    assert.equal(running, false, 'owned grandchild survived launcher cleanup');
  } finally {
    cleanup();
  }
});
