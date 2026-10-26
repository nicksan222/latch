import assert from 'node:assert/strict';
import test from 'node:test';
import { publishRelease } from './publish-release.mjs';
const sha = 'a'.repeat(40);
const assets = ['SHA256SUMS', 'example.deb'];
function fixture({ commit = sha, release = null } = {}) {
  const calls = [];
  const run = (args) => {
    calls.push(args);
    if (args[0] === 'api') {
      if (args[1].includes('/git/ref/tags/')) return JSON.stringify({ ref: 'refs/tags/v1.0.0' });
      if (args[1].includes('/commits/')) return JSON.stringify({ sha: commit });
      if (release) return JSON.stringify(release);
      throw new Error('HTTP 404: Not Found');
    }
    return '';
  };
  return { calls, run };
}
test('creates draft, uploads verified assets, then publishes', () => {
  const f = fixture();
  publishRelease({ run: f.run, repository: 'owner/repo', tag: 'v1.0.0', sha, assets });
  assert.deepEqual(
    f.calls.filter((args) => args[0] === 'release').map((args) => args[1]),
    ['create', 'upload', 'edit']
  );
  assert.ok(f.calls.find((args) => args[1] === 'create').includes('--draft'));
});
test('rerun leaves a complete published release unchanged', () => {
  const f = fixture({ release: { draft: false, assets: assets.map((name) => ({ name })) } });
  publishRelease({ run: f.run, repository: 'owner/repo', tag: 'v1.0.0', sha, assets });
  assert.equal(f.calls.filter((args) => args[0] === 'release').length, 0);
});
test('resumes an existing draft', () => {
  const f = fixture({ release: { draft: true } });
  publishRelease({ run: f.run, repository: 'owner/repo', tag: 'v1.0.0', sha, assets });
  assert.deepEqual(
    f.calls.filter((args) => args[0] === 'release').map((args) => args[1]),
    ['upload', 'edit']
  );
});
test('fails closed on retargeted tag, incomplete published release and authorization failures', () => {
  for (const f of [
    fixture({ commit: 'b'.repeat(40) }),
    fixture({ release: { draft: false, assets: [] } }),
    {
      run: () => {
        throw new Error('HTTP 403');
      }
    }
  ]) {
    assert.throws(() =>
      publishRelease({ run: f.run, repository: 'owner/repo', tag: 'v1.0.0', sha, assets })
    );
  }
});

test('new automatic tag may be absent before draft creation but must exist before publishing', () => {
  let created = false;
  const calls = [];
  publishRelease({
    repository: 'owner/repo',
    tag: 'v1.0.0',
    sha,
    assets,
    run: (args) => {
      calls.push(args);
      if (args[0] === 'api') {
        if (args[1].endsWith('/git/refs')) {
          created = true;
          return '';
        }
        if (args[1].includes('/git/ref/tags/') && created)
          return JSON.stringify({ ref: 'refs/tags/v1.0.0' });
        if (args[1].includes('/commits/')) {
          if (created) return JSON.stringify({ sha });
          throw new Error('gh: No commit found for SHA: v1.0.0 (HTTP 422)');
        }
        throw new Error('HTTP 404: Not Found');
      }
      if (args[1] === 'create') created = true;
      return '';
    }
  });
  assert.equal(calls.at(-1)[1], 'edit');
  assert.equal(calls[0][1], 'repos/owner/repo/git/ref/tags/v1.0.0');
  const reservation = calls.findIndex((args) => args[1]?.endsWith('/git/refs'));
  const draft = calls.findIndex((args) => args[1] === 'create');
  assert.ok(reservation >= 0 && reservation < draft);
  assert.ok(calls[reservation].includes(`sha=${sha}`));
});

test('publication refuses malformed tags before invoking gh', () => {
  let called = false;
  assert.throws(() =>
    publishRelease({
      run: () => {
        called = true;
      },
      repository: 'owner/repo',
      tag: 'v1.0.0-rc.1',
      sha,
      assets
    })
  );
  assert.equal(called, false);
});

test('Linux artifact contract requires exactly CLI, deb, AppImage and checksums', async () => {
  const { validateArtifactNames } = await import('./publish-release.mjs');
  const names = [
    'latch-0.1.1-x86_64-unknown-linux-gnu.tar.gz',
    'Latch_0.1.1_amd64.deb',
    'Latch_0.1.1_amd64.AppImage',
    'SHA256SUMS'
  ];
  assert.doesNotThrow(() => validateArtifactNames(names));
  for (const invalid of [
    names.slice(1),
    [...names, 'extension.zip'],
    [...names.slice(0, 3), 'other.tar.gz'],
    [...names.slice(0, 3), '../SHA256SUMS']
  ]) {
    assert.throws(() => validateArtifactNames(invalid));
  }
});

test('an existing tag lookup failure never becomes permission to create or publish', () => {
  for (const failure of ['HTTP 422: Validation failed', 'HTTP 403: Forbidden']) {
    const calls = [];
    assert.throws(
      () =>
        publishRelease({
          repository: 'owner/repo',
          tag: 'v1.0.0',
          sha,
          assets,
          run: (args) => {
            calls.push(args);
            if (args[1].includes('/git/ref/tags/'))
              return JSON.stringify({ ref: 'refs/tags/v1.0.0' });
            throw new Error(failure);
          }
        }),
      new RegExp(failure)
    );
    assert.ok(calls.every((args) => args[0] === 'api' && args.length === 2));
  }
});
