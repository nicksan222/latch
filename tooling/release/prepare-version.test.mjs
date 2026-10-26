import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocuments } from './prepare-version.mjs';
const fixture = {
  'package.json': JSON.stringify({ version: '0.1.0' }),
  'bun.lock': `{
  "lockfileVersion": 2,
  "configVersion": 0,
  "workspaces": {
    "": { "name": "latch-workspace" },
    "packages/foo": { "name": "@latch/foo", "version": "0.1.0" },
  },
  "packages": { "foo": ["foo@9.0.0", "", {}, "sha512-fixture"] },
}\n`,
  'Cargo.toml': '[workspace.package]\nversion = "0.1.0"\nedition = "2024"\n',
  'Cargo.lock':
    'version = 4\n\n[[package]]\nname = "latch-core"\nversion = "0.1.0"\n\n[[package]]\nname = "external"\nversion = "0.1.0"\n',
  'apps/desktop/src-tauri/tauri.conf.json': JSON.stringify({ version: '0.1.0' })
};
test('prepares root manifests, Cargo workspace/lock and Tauri without changing dependencies', () => {
  const result = prepareDocuments(fixture, '2.3.4');
  assert.equal(JSON.parse(result['package.json']).version, '2.3.4');
  assert.equal(result['bun.lock'], fixture['bun.lock']);
  assert.match(result['Cargo.toml'], /version = "2.3.4"/);
  assert.match(result['Cargo.lock'], /name = "latch-core"\nversion = "2.3.4"/);
  assert.match(result['Cargo.lock'], /name = "external"\nversion = "0.1.0"/);
  assert.equal(JSON.parse(result['apps/desktop/src-tauri/tauri.conf.json']).version, '2.3.4');
});
test('refuses malformed versions and absent workspace version instead of partial preparation', () => {
  assert.throws(() => prepareDocuments(fixture, '2.3.4-rc'));
  assert.throws(() => prepareDocuments({ ...fixture, 'Cargo.toml': '[workspace]\n' }, '2.3.4'));
});

test('does not accept a version in a later TOML section or malformed existing version', () => {
  assert.throws(() =>
    prepareDocuments(
      {
        ...fixture,
        'Cargo.toml': '[workspace.package]\nedition = "2024"\n[package]\nversion = "0.1.0"\n'
      },
      '1.0.0'
    )
  );
  assert.throws(() =>
    prepareDocuments(
      { ...fixture, 'Cargo.toml': '[workspace.package]\nversion = "invalid"\n' },
      '1.0.0'
    )
  );
  assert.throws(() =>
    prepareDocuments(
      {
        ...fixture,
        'Cargo.lock': fixture['Cargo.lock'].replace('version = "0.1.0"', 'version = "invalid"')
      },
      '1.0.0'
    )
  );
});

test('prepares the dev-only bridge lock version and accepts Linux versions beyond extension limits', () => {
  const documents = {
    ...fixture,
    'Cargo.lock':
      fixture['Cargo.lock'] + '\n[[package]]\nname = "latch-web-bridge"\nversion = "0.1.0"\n'
  };
  const result = prepareDocuments(documents, '1.2.65536');
  assert.match(result['Cargo.lock'], /name = "latch-web-bridge"\nversion = "1.2.65536"/);
  assert.equal(JSON.parse(result['apps/desktop/src-tauri/tauri.conf.json']).version, '1.2.65536');
});

test('keeps the standalone RPC interface lock version in sync with its workspace', () => {
  const documents = {
    ...fixture,
    'Cargo.lock': fixture['Cargo.lock'] + '\n[[package]]\nname = "latch-rpc"\nversion = "0.1.0"\n'
  };
  const result = prepareDocuments(documents, '0.1.1');
  assert.match(result['Cargo.lock'], /name = "latch-rpc"\nversion = "0.1.1"/);
});

test('prepares the Rust automation workspace lock version for locked release builds', () => {
  const documents = {
    ...fixture,
    'Cargo.lock':
      fixture['Cargo.lock'] + '\n[[package]]\nname = "latch-automation"\nversion = "0.1.0"\n'
  };
  const result = prepareDocuments(documents, '0.1.1');
  assert.match(result['Cargo.lock'], /name = "latch-automation"\nversion = "0.1.1"/);
});
