import assert from 'node:assert/strict';
import { test } from 'node:test';
test('separate browser invocations get isolated artifacts and workers keep their run identity', async () => {
  const { selectE2ERun } = await import('./prerequisites.ts');
  const firstEnv = {};
  const first = selectE2ERun(firstEnv);
  assert.equal(selectE2ERun(firstEnv).id, first.id);
  assert.notEqual(selectE2ERun({}).id, first.id);
  assert.equal(first.port, 1423);
  assert.deepEqual(selectE2ERun({ LATCH_E2E_RUN_ID: 'proof-a', LATCH_E2E_PORT: '1424' }), {
    id: 'proof-a',
    port: 1424
  });
});
test('artifact IDs cannot escape the safe output directory', async () => {
  const { selectE2ERun } = await import('./prerequisites.ts');
  for (const id of ['../latch/vault', '/tmp/elsewhere', 'a/b', 'a\\b', '.', '']) {
    assert.throws(() => selectE2ERun({ LATCH_E2E_RUN_ID: id }), /Invalid E2E run ID/);
  }
});
test('test ports must be valid unprivileged integers', async () => {
  const { selectE2ERun } = await import('./prerequisites.ts');
  for (const port of ['abc', '1423; command', '80', '65536', '1423.5']) {
    assert.throws(() => selectE2ERun({ LATCH_E2E_PORT: port }), /Invalid E2E port/);
  }
});
