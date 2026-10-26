import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveSite, withBase } from './site.mjs';
test('repository Pages URL keeps project base', () => {
  assert.deepEqual(resolveSite({ GITHUB_REPOSITORY: 'alice/latch' }), { site: 'https://alice.github.io', base: '/latch', repository: 'https://github.com/alice/latch' });
});
test('user Pages and custom URL have no project base', () => {
  assert.equal(resolveSite({ GITHUB_REPOSITORY: 'alice/alice.github.io' }).base, '/');
  assert.deepEqual(resolveSite({ LANDING_SITE: 'https://secrets.example/team/', GITHUB_REPOSITORY: 'alice/latch' }), { site: 'https://secrets.example', base: '/team', repository: 'https://github.com/alice/latch' });
});
test('local default includes release destination and base helper avoids double slashes', () => {
  assert.equal(resolveSite({}).repository, 'https://github.com/nicksan222/latch');
  assert.equal(withBase('/latch/', '/demo/'), '/latch/demo/');
  assert.equal(withBase('/', '/demo/'), '/demo/');
});
