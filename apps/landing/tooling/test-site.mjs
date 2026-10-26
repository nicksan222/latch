import { spawnSync } from 'node:child_process';
// Browser fixtures exercise root URLs. The deployment workflow separately builds
// and verifies the real Pages base; CI's GITHUB_REPOSITORY must not change these fixtures.
const env = { ...process.env, LANDING_SITE: 'http://localhost:4321/', LANDING_BASE: '/' };
const result = spawnSync('npm', ['run', 'test:site', '--workspace', '@latch/landing'], { env, stdio: 'inherit', cwd: new URL('../../..', import.meta.url) });
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
