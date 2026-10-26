import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';

function kill(pid) {
  if (!pid) return;
  try {
    process.kill(pid, 'SIGKILL');
  } catch (error) {
    if (error.code !== 'ESRCH') throw error;
  }
}
function running(pid) {
  try {
    return !readFileSync(`/proc/${pid}/stat`, 'utf8').split(') ')[1].startsWith('Z');
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  test(
    `E2E ${signal} preserves controller cleanup`,
    { skip: process.platform !== 'linux', timeout: 10_000 },
    async (t) => {
      const folder = mkdtempSync(join(tmpdir(), 'latch-e2e-runner-'));
      // The fake controller takes time to clean up its stubborn descendant,
      // just as Playwright must tear down separately owned browser/server groups.
      const cli = join(folder, 'controller.cjs');
      const marker = join(folder, 'cleaned');
      writeFileSync(
        cli,
        `const {spawn}=require('node:child_process');
const {writeFileSync}=require('node:fs');
const child=spawn(process.execPath,['-e',"process.on('SIGTERM',()=>{});process.on('SIGINT',()=>{});setInterval(()=>{},1000)"],{stdio:'ignore'});
process.on('SIGINT',()=>setTimeout(()=>{child.kill('SIGKILL');writeFileSync(${JSON.stringify(marker)},'clean');process.exit(130)},100));
console.log(child.pid);
setInterval(()=>{},1000);
`
      );
      const entry = join(folder, 'entry.mjs');
      writeFileSync(
        entry,
        `import {runPlaywright} from ${JSON.stringify(new URL('./run.mjs', import.meta.url).href)};process.exitCode=await runPlaywright([],${JSON.stringify(cli)});`
      );
      const wrapper = spawn(process.execPath, [entry], { stdio: ['ignore', 'pipe', 'inherit'] });
      const exited = once(wrapper, 'exit', { signal: t.signal });
      let descendant;
      try {
        const [data] = await once(wrapper.stdout, 'data', { signal: t.signal });
        descendant = Number(data.toString().trim());
        assert.ok(Number.isSafeInteger(descendant) && descendant > 1);
        wrapper.kill(signal);
        const [code] = await exited;
        assert.equal(code, 130);
        assert.equal(readFileSync(marker, 'utf8'), 'clean');
        const deadline = Date.now() + 1000;
        while (running(descendant) && Date.now() < deadline) await delay(20);
        assert.equal(running(descendant), false, 'E2E left an owned descendant running');
      } finally {
        kill(wrapper.pid);
        kill(descendant);
        rmSync(folder, { recursive: true, force: true });
      }
    }
  );
}
