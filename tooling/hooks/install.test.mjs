import assert from 'node:assert/strict';
import test from 'node:test';
import { assertHookPath } from './policy.mjs';

test('unconfigured and already configured repositories can opt in', () => {
  assert.equal(assertHookPath(''), 'tooling/hooks');
  assert.equal(assertHookPath('tooling/hooks'), 'tooling/hooks');
});

test('existing custom hooks are never silently replaced', () => {
  for (const path of ['.husky', '/home/developer/shared-hooks', '../hooks']) {
    assert.throws(() => assertHookPath(path), /already configured differently/);
  }
});
