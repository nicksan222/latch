import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { test } from 'node:test';
import { featureCommands, validateFeatures } from './features.mjs';

const features = [
  {
    id: 'secrets',
    sources: ['packages/core/secret.rs'],
    rust: [{ package: 'latch-cli', target: 'features', filter: 'secrets::' }],
    browser: []
  },
  {
    id: 'profiles',
    sources: ['packages/core/profile.rs'],
    rust: [{ package: 'latch-cli', target: 'features', filter: 'profiles::' }],
    browser: []
  }
];

test('feature selection cannot silently run zero or unrelated scenarios', () => {
  assert.throws(() => featureCommands(features, ['secret']), /Unknown feature/);
  assert.throws(() => featureCommands(features, []), /Select at least one/);
  assert.deepEqual(featureCommands(features, ['secrets']), [
    [
      'test',
      '--locked',
      '--no-default-features',
      '-p',
      'latch-cli',
      '--test',
      'features',
      'secrets::'
    ]
  ]);
});

test('feature ownership requires unique identifiers and existing source and browser paths', () => {
  assert.deepEqual(
    validateFeatures(features, () => true),
    []
  );
  assert.ok(
    validateFeatures([...features, features[0]], () => true).some((error) =>
      /Duplicate/.test(error)
    )
  );
  assert.ok(validateFeatures(features, () => false).some((error) => /Missing/.test(error)));
  assert.ok(
    validateFeatures([{ ...features[0], rust: [] }], () => true).some((error) => /Rust/.test(error))
  );
});

test('multiple feature selections are deduplicated without dropping scenario filters', () => {
  assert.equal(featureCommands(features, ['secrets', 'profiles', 'secrets']).length, 2);
});

// Exercise the real command entrypoint: Cargo's zero-match success must fail CI.
for (const [discovery, expectedStatus, executes] of [
  ['empty', 1, false],
  ['failed', 17, false],
  ['found', 0, true]
]) {
  test(
    `runner handles ${discovery} discovery before executing a feature`,
    { skip: process.platform === 'win32' },
    () => {
      const folder = mkdtempSync(join(tmpdir(), 'latch-feature-runner-'));
      const marker = join(folder, 'executed');
      try {
        const cargo = join(folder, 'cargo');
        writeFileSync(
          cargo,
          `#!/usr/bin/env node
const fs = require('node:fs');
if (process.argv.includes('--list')) {
  if (${JSON.stringify(discovery)} === 'failed') process.exit(17);
  if (${JSON.stringify(discovery)} === 'found') console.log('feature_scenario: test');
} else fs.writeFileSync(${JSON.stringify(marker)}, 'executed');
`
        );
        chmodSync(cargo, 0o700);
        const result = spawnSync(
          process.execPath,
          [new URL('./features.mjs', import.meta.url).pathname, 'git'],
          {
            env: { ...process.env, PATH: `${folder}${delimiter}${process.env.PATH}` },
            encoding: 'utf8'
          }
        );
        assert.equal(result.status, expectedStatus, result.stderr);
        assert.equal(existsSync(marker), executes);
        if (discovery === 'empty') assert.match(result.stderr, /No tests discovered/);
      } finally {
        rmSync(folder, { recursive: true, force: true });
      }
    }
  );
}

test('library feature scenarios retain their module filter', () => {
  const otp = [
    { id: 'totp', rust: [{ package: 'latch-core', kind: 'lib', filter: 'core::totp::tests' }] }
  ];
  assert.deepEqual(featureCommands(otp, ['totp']), [
    ['test', '--locked', '--no-default-features', '-p', 'latch-core', '--lib', 'core::totp::tests']
  ]);
});

// Domain acceptance must remain executable without building a consumer app.
test('RPC and shared CRUD acceptance are core-owned', () => {
  const inventory = JSON.parse(readFileSync(new URL('./features.json', import.meta.url), 'utf8'));
  for (const id of ['rpc', 'profiles', 'secrets']) {
    const feature = inventory.find((item) => item.id === id);
    assert.ok(feature, `Missing core contract owner: ${id}`);
    assert.ok(feature.rust.length > 0);
    assert.ok(feature.rust.every((suite) => suite.package === 'latch-core'));
  }
  const rpc = inventory.find((item) => item.id === 'rpc');
  assert.deepEqual(rpc.rust, [{ package: 'latch-core', target: 'rpc_external' }]);
  assert.ok(rpc.acceptance.every((path) => path.includes('/rpc/tests/external/')));
});
