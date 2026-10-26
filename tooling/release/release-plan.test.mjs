import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { planRelease, readRepositoryTags, parseVersion } from './release-plan.mjs';

const sha = 'a'.repeat(40);
const other = 'b'.repeat(40);
const base = { sourceVersion: '0.1.0', sha, tags: [], step: 'patch' };
test('stable semver rejects coercion, leading zeros, metadata, prereleases and overflow', () => {
  for (const version of [
    '1.2',
    '01.2.3',
    'v1.2.3',
    '1.2.3-beta',
    '1.2.3+build',
    '1.2.3\n',
    '9007199254740992.0.0',
    null
  ]) {
    assert.throws(() => parseVersion(version));
  }
});
test('automatic policy increments max(source, numeric highest tag)', () => {
  const tags = [
    { tag: 'v0.9.9', sha: other },
    { tag: 'v0.10.1', sha: other }
  ];
  assert.equal(planRelease({ ...base, tags }).version, '0.10.2');
  assert.equal(planRelease({ ...base, tags, step: 'minor' }).version, '0.11.0');
  assert.equal(planRelease({ ...base, sourceVersion: '2.0.0', tags }).version, '2.0.1');
});
test('missing policy and malformed release tags fail closed', () => {
  assert.throws(() => planRelease({ ...base, step: undefined }));
  assert.throws(() => planRelease({ ...base, step: 'major' }));
  for (const tag of ['v1.02.3', 'v1.2', 'v2.0.0-rc.1']) {
    assert.throws(() => planRelease({ ...base, tags: [{ tag, sha: other }] }));
  }
  assert.throws(() => planRelease({ ...base, sourceVersion: 'no' }));
});
test('explicit stable tag is authoritative and must resolve to tested commit', () => {
  const tags = [
    { tag: 'v4.0.0', sha },
    { tag: 'v5.0.0', sha: other }
  ];
  assert.equal(planRelease({ ...base, tags, explicitTag: 'v4.0.0' }).version, '4.0.0');
  for (const explicitTag of ['4.0.0', 'v4.0.0-rc', 'v3.0.0', 'v5.0.0']) {
    assert.throws(() => planRelease({ ...base, tags, explicitTag }));
  }
});
test('main rerun reuses highest tag at the same SHA, even after newer releases', () => {
  const tags = [
    { tag: 'v1.1.0', sha },
    { tag: 'v2.0.0', sha },
    { tag: 'v3.0.0', sha: other }
  ];
  assert.equal(planRelease({ ...base, tags }).tag, 'v2.0.0');
});
test('Linux release versions are not constrained by removed browser extension limits', () => {
  assert.equal(planRelease({ ...base, sourceVersion: '1.2.65535' }).version, '1.2.65536');
  assert.equal(planRelease({ ...base, sourceVersion: '65536.0.0' }).version, '65536.0.1');
  assert.throws(() => planRelease({ ...base, sourceVersion: '1.2.9007199254740991' }));
});
test('repository reader peels annotated tags and ignores unrelated tags', () => {
  const dir = mkdtempSync(join(tmpdir(), 'latch-release-test-'));
  const git = (...args) =>
    execFileSync(
      'git',
      [
        '-c',
        'core.hooksPath=/dev/null',
        '-c',
        'commit.gpgsign=false',
        '-c',
        'tag.gpgsign=false',
        '-C',
        dir,
        ...args
      ],
      {
        encoding: 'utf8',
        env: {
          ...Object.fromEntries(
            Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_'))
          ),
          GIT_CONFIG_NOSYSTEM: '1',
          GIT_CONFIG_GLOBAL: '/dev/null'
        }
      }
    ).trim();
  try {
    git('init', '--quiet');
    git('config', 'user.email', 'test@example.invalid');
    git('config', 'user.name', 'Release test');
    writeFileSync(join(dir, 'fixture'), 'nonsecret');
    git('add', 'fixture');
    git('commit', '--quiet', '-m', 'fixture');
    git('tag', '-a', 'v1.0.0', '-m', 'fixture');
    git('tag', 'documentation');
    assert.deepEqual(readRepositoryTags(dir), [{ tag: 'v1.0.0', sha: git('rev-parse', 'HEAD') }]);
    git('tag', 'vbroken');
    assert.throws(() => planRelease({ ...base, tags: readRepositoryTags(dir) }));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
