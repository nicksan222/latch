import assert from 'node:assert/strict';
import { test } from 'node:test';
import { frontendArguments } from './frontend.mjs';

test('browser development serves the desktop entry and validates ports', () => {
  assert.deepEqual(frontendArguments([]), {
    port: 1422,
    args: ['-C', 'apps/desktop', 'dev', '--port', '1422']
  });
  assert.equal(frontendArguments(['--port', '1424']).port, 1424);
  for (const port of ['0', '80', '65536', '1422;echo', 'NaN']) {
    assert.throws(() => frontendArguments(['--port', port]), /Invalid frontend port/);
  }
  assert.throws(() => frontendArguments(['--host', '0.0.0.0']), /Unsupported frontend arguments/);
});
