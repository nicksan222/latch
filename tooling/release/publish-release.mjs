import { execFileSync } from 'node:child_process';
import { lstatSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseVersion } from './release-plan.mjs';

function optionalJson(run, endpoint) {
  try {
    return JSON.parse(run(['api', endpoint]));
  } catch (error) {
    if (/HTTP 404\b/.test(String(error.stderr ?? error.message))) return null;
    throw error;
  }
}

export function publishRelease({ run, repository, tag, sha, assets }) {
  if (typeof tag !== 'string' || !tag.startsWith('v'))
    throw new Error('Expected stable release tag');
  parseVersion(tag.slice(1));
  if (!/^[a-f0-9]{40}$/.test(sha) || !/^[\w.-]+\/[\w.-]+$/.test(repository))
    throw new Error('Invalid release target');
  const commit = optionalJson(run, `repos/${repository}/commits/${tag}`);
  if (commit && commit.sha !== sha) throw new Error('Release tag changed since checks');
  const release = optionalJson(run, `repos/${repository}/releases/tags/${tag}`);
  if (release && !commit) throw new Error('Release tag is missing');
  if (release && !release.draft) {
    if (assets.some((name) => !release.assets?.some((asset) => asset.name === name)))
      throw new Error('Published release is missing assets; manual investigation required');
    return;
  }
  // Draft releases may defer tag creation until publication. Reserve the exact
  // tested SHA first, using GITHUB_TOKEN only in this publish job.
  if (!commit)
    run([
      'api',
      `repos/${repository}/git/refs`,
      '--method',
      'POST',
      '-f',
      `ref=refs/tags/${tag}`,
      '-f',
      `sha=${sha}`
    ]);
  if (!release)
    run([
      'release',
      'create',
      tag,
      '--repo',
      repository,
      '--target',
      sha,
      '--draft',
      '--title',
      `🔐 Latch ${tag}`,
      '--generate-notes'
    ]);
  run([
    'release',
    'upload',
    tag,
    ...assets.map((name) => `release-artifacts/${name}`),
    '--repo',
    repository,
    '--clobber'
  ]);
  // Recheck after upload before exposing the draft to users.
  const finalCommit = optionalJson(run, `repos/${repository}/commits/${tag}`);
  if (!finalCommit || finalCommit.sha !== sha)
    throw new Error('Release tag does not match tested commit');
  run(['release', 'edit', tag, '--repo', repository, '--draft=false']);
}

export function validateArtifactNames(assets) {
  if (
    assets.length !== 4 ||
    new Set(assets).size !== 4 ||
    assets.some((name) => !/^[\w.+-]+$/.test(name)) ||
    !assets.includes('SHA256SUMS') ||
    assets.filter((name) => /^latch-.*-x86_64-unknown-linux-gnu\.tar\.gz$/.test(name)).length !==
      1 ||
    assets.filter((name) => name.endsWith('.deb')).length !== 1 ||
    assets.filter((name) => name.endsWith('.AppImage')).length !== 1
  ) {
    throw new Error('Expected CLI archive, deb, AppImage and SHA256SUMS');
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const assets = readdirSync('release-artifacts').sort();
  validateArtifactNames(assets);
  if (assets.some((name) => !lstatSync(`release-artifacts/${name}`).isFile()))
    throw new Error('Unexpected release artifact type');
  execFileSync('sha256sum', ['--check', 'SHA256SUMS'], {
    cwd: 'release-artifacts',
    stdio: 'inherit'
  });
  publishRelease({
    run: (args) =>
      execFileSync('gh', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }),
    repository: process.env.GH_REPO,
    tag: process.env.RELEASE_TAG,
    sha: process.env.RELEASE_SHA,
    assets
  });
}
