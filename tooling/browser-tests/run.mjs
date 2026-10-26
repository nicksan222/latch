import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import { join } from 'node:path';

const cache =
  process.env.LATCH_NATIVE_BROWSER_CACHE ?? join(homedir(), '.cache/latch-browser-tests');
const prepare = spawnSync(
  'cargo',
  [
    'run',
    '--locked',
    '-q',
    '-p',
    'latch-core',
    '--no-default-features',
    '--example',
    'managed_browser',
    '--',
    cache
  ],
  { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }
);
if (prepare.status !== 0) process.exit(prepare.status ?? 1);
const executable = prepare.stdout.trim();
if (!executable) throw new Error('Managed Firefox installer returned no executable');
const rpc = spawnSync(
  'cargo',
  [
    'test',
    '--locked',
    '-q',
    '-p',
    'latch-core',
    '--no-default-features',
    '--test',
    'rpc_external',
    'native_browser::managed_firefox',
    '--',
    '--ignored'
  ],
  { stdio: 'inherit' }
);
if (rpc.status !== 0) process.exit(rpc.status ?? 1);
const test = spawnSync(process.execPath, ['--test', 'tooling/browser-tests/native.test.mjs'], {
  stdio: 'inherit',
  env: { ...process.env, LATCH_TEST_FIREFOX: executable }
});
process.exit(test.status ?? 1);
