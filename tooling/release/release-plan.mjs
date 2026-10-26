import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function parseVersion(value) {
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value)) {
    throw new Error('Expected strict stable semver X.Y.Z');
  }
  const parts = value.split('.').map(Number);
  if (parts.some((part) => !Number.isSafeInteger(part)))
    throw new Error('Version component overflow');
  return parts;
}

function compare(a, b) {
  const x = parseVersion(a);
  const y = parseVersion(b);
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
}

export function readRepositoryTags(directory = '.') {
  const git = (...args) =>
    execFileSync('git', ['-C', directory, ...args], {
      encoding: 'utf8',
      env: Object.fromEntries(
        Object.entries(process.env).filter(
          ([key]) => !['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE'].includes(key)
        )
      )
    }).trim();
  const names = git('tag', '--list', 'v*');
  return names
    ? names
        .split('\n')
        .map((tag) => ({ tag, sha: git('rev-parse', '--verify', `refs/tags/${tag}^{commit}`) }))
    : [];
}

export function planRelease({ sourceVersion, sha, tags, step, explicitTag }) {
  parseVersion(sourceVersion);
  if (!/^[a-f0-9]{40}$/.test(sha)) throw new Error('Expected tested commit SHA');
  if (!['patch', 'minor'].includes(step))
    throw new Error('Release policy must select patch or minor');
  for (const item of tags) {
    if (!item.tag.startsWith('v')) throw new Error('Expected vX.Y.Z release tag');
    parseVersion(item.tag.slice(1));
    if (!/^[a-f0-9]{40}$/.test(item.sha)) throw new Error('Invalid tag commit');
  }
  const sorted = [...tags].sort((a, b) => compare(b.tag.slice(1), a.tag.slice(1)));
  let version;
  let existing = false;
  if (explicitTag !== undefined) {
    if (!explicitTag.startsWith('v')) throw new Error('Expected vX.Y.Z explicit tag');
    parseVersion(explicitTag.slice(1));
    if (!tags.some((item) => item.tag === explicitTag && item.sha === sha))
      throw new Error('Explicit tag does not point to tested commit');
    version = explicitTag.slice(1);
    existing = true;
  } else {
    const sameCommit = sorted.find((item) => item.sha === sha);
    if (sameCommit) {
      version = sameCommit.tag.slice(1);
      existing = true;
    } else {
      const latest = sorted[0]?.tag.slice(1);
      const parts = parseVersion(
        latest && compare(latest, sourceVersion) > 0 ? latest : sourceVersion
      );
      if (step === 'minor') {
        parts[1]++;
        parts[2] = 0;
      } else parts[2]++;
      version = parts.join('.');
    }
  }
  parseVersion(version);
  return { version, tag: `v${version}`, sha, existing };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const policy = JSON.parse(readFileSync('tooling/release/policy.json', 'utf8'));
  const source = JSON.parse(readFileSync('package.json', 'utf8'));
  const plan = planRelease({
    sourceVersion: source.version,
    sha: process.env.GITHUB_SHA,
    tags: readRepositoryTags(),
    step: policy.step,
    ...(process.env.GITHUB_REF_TYPE === 'tag' ? { explicitTag: process.env.GITHUB_REF_NAME } : {})
  });
  if (process.env.GITHUB_OUTPUT)
    appendFileSync(
      process.env.GITHUB_OUTPUT,
      `version=${plan.version}\ntag=${plan.tag}\nsha=${plan.sha}\n`
    );
  console.log(JSON.stringify(plan));
}
