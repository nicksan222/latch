import assert from 'node:assert/strict';
import { stripTypeScriptTypes } from 'node:module';
import { test } from 'node:test';
import { generateDesktopSdk } from './sdk.mjs';

const procedure = (id, name, paramsMode, request, result, imports = []) => ({
  id,
  name,
  paramsMode,
  request: { typescript: request, proto: request === 'void' ? 'Empty' : request },
  result: { typescript: result, proto: result === 'null' ? 'Empty' : result },
  imports
});

const procedures = [
  procedure('VAULT_STATUS', 'vault.status', 'none', 'void', 'VaultStatus', ['VaultStatus']),
  procedure('VAULT_TRUST', 'vault.trust', 'empty_object', 'Record<string, never>', 'TrustStatus', [
    'TrustStatus'
  ]),
  procedure('ENTRIES_CREATE', 'entries.create', 'typed', 'EntryInput', 'Entry', [
    'Entry',
    'EntryInput'
  ])
];

test('desktop compatibility SDK preserves legacy names, parameter modes and result shapes', async () => {
  const desktop = generateDesktopSdk({ version: 1, procedures });
  const executable = stripTypeScriptTypes(desktop.replace(/^import .*;\n/gm, ''));
  const { createDesktopSdk } = await import(
    `data:text/javascript,${encodeURIComponent(executable)}`
  );
  const calls = [];
  const sdk = createDesktopSdk(async (...args) => {
    calls.push(args);
    return 'result';
  });
  assert.equal(await sdk.vault.status(), 'result');
  assert.equal(await sdk.vault.trust(), 'result');
  assert.equal(await sdk.entries.create({ name: 'example' }), 'result');
  assert.deepEqual(
    calls.map(([, args]) => args.params),
    [null, {}, { name: 'example' }]
  );
  assert.match(desktop, /status: \(\)/);
  assert.match(desktop, /trust: \(\)/);
  assert.match(desktop, /create: \(params: EntryInput\)/);
  await assert.rejects(() => sdk.call('toString', {}), /Unknown RPC procedure/);
});

test('missing, empty and duplicate procedure metadata fails closed', () => {
  for (const manifest of [
    {},
    { version: 1, procedures: [] },
    { version: 1, procedures: [procedures[0], procedures[0]] },
    { version: 1, procedures: [{ ...procedures[0], id: '' }] }
  ]) {
    assert.throws(() => generateDesktopSdk(manifest), /procedure/i);
  }
});

test('procedure names cannot overwrite namespaces in either declaration order', () => {
  const nested = procedure('FOO_BAR', 'foo.bar', 'none', 'void', 'null');
  const root = procedure('FOO', 'foo', 'none', 'void', 'null');
  for (const order of [
    [nested, root],
    [root, nested]
  ]) {
    assert.throws(() => generateDesktopSdk({ version: 1, procedures: order }), /namespace/i);
  }
});
