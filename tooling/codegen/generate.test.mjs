import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { test } from 'node:test';
import { compareFiles, defaultOutputTargets, generateBindings } from './generate.mjs';
import { compileProto, generateProtoOutputs } from './protobuf.mjs';

test('default outputs place generated protobuf contracts in their owning packages', () => {
  const targets = defaultOutputTargets();
  assert.match(targets.packageTypes, /packages\/rpc\/ts\/?$/);
  assert.match(targets.contracts, /apps\/desktop\/src\/_generated\/contracts\/?$/);
  assert.match(targets.desktopSdk, /apps\/desktop\/src\/sdk\/generated\.ts$/);
  assert.match(targets.manifest, /packages\/core\/_generated\/procedures\.json$/);
});

test('freshness detects changed, missing and stale generated files without rewriting them', () => {
  const root = mkdtempSync(join(tmpdir(), 'latch-proto-freshness-'));
  try {
    const actual = join(root, 'actual');
    const expected = join(root, 'expected');
    mkdirSync(actual);
    mkdirSync(expected);
    writeFileSync(join(actual, 'Changed.ts'), 'before');
    writeFileSync(join(expected, 'Changed.ts'), 'after');
    writeFileSync(join(actual, 'Stale.ts'), 'stale');
    writeFileSync(join(expected, 'New.ts'), 'new');
    assert.deepEqual(compareFiles(actual, expected), ['Changed.ts', 'New.ts', 'Stale.ts']);
    assert.equal(readFileSync(join(actual, 'Changed.ts'), 'utf8'), 'before');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('protoc descriptor drives compatibility types and the complete unary procedure manifest', () => {
  const { descriptor } = compileProto({ protoc: process.env.PROTOC ?? 'protoc' });
  const { types, manifest } = generateProtoOutputs(descriptor);
  assert.ok(manifest.procedures.length >= 61);
  assert.equal(manifest.procedures.length, descriptor.service[0].method.length - 1); // Excludes streaming WatchEvents.
  assert.ok(
    descriptor.service[0].method.some(
      (method) => method.name === 'WatchEvents' && method.serverStreaming
    )
  );
  assert.ok(
    types.find(({ name }) => name === 'EntryPayload.ts').source.includes('"type": "ssh_key"')
  );
  assert.ok(
    types
      .find(({ name }) => name === 'DomainRequest.ts')
      .source.includes('"operation": "merge_entries"')
  );
  assert.ok(
    types
      .find(({ name }) => name === 'DomainResponse.ts')
      .source.includes('"output": DomainRecipientKeyKind | null')
  );
  assert.ok(types.find(({ name }) => name === 'TotpAlgorithm.ts').source.includes('"SHA1"'));
  assert.ok(!types.find(({ name }) => name === 'StringMap.ts').source.includes('ItemsEntry'));
  assert.ok(
    types
      .find(({ name }) => name === 'ClipboardCopyParams.ts')
      .source.includes('"clearAfterSecs"?: number | null')
  );
  assert.ok(
    types
      .find(({ name }) => name === 'AgentEvent.ts')
      .source.includes('"profileId"?: string | null')
  );
  assert.deepEqual(
    manifest.procedures
      .slice(0, 3)
      .map(({ name, paramsMode, request, result }) => [
        name,
        paramsMode,
        request.proto,
        result.proto
      ]),
    [
      ['domain.call', 'typed', 'DomainRequest', 'DomainResponse'],
      ['hello', 'typed', 'HelloParams', 'HelloResult'],
      ['vault.status', 'none', 'Empty', 'VaultStatus']
    ]
  );
});

test('generation is atomic, keeps a separate core catalog export, and checks all protobuf-derived outputs', () => {
  const root = mkdtempSync(join(tmpdir(), 'latch-proto-generate-'));
  try {
    const targets = {
      packageTypes: `${join(root, 'rpc/ts')}${sep}`,
      contracts: `${join(root, 'desktop/contracts')}${sep}`,
      desktopSdk: join(root, 'desktop/sdk/generated.ts'),
      manifest: join(root, 'core/procedures.json')
    };
    const exportCatalog = ({ env }) => {
      mkdirSync(env.LATCH_CATALOG_EXPORT_DIR, { recursive: true });
      writeFileSync(
        join(env.LATCH_CATALOG_EXPORT_DIR, 'domain-catalog.json'),
        '{"entryTypes":[]}\n'
      );
    };
    const compile = (options) =>
      compileProto({ ...options, protoc: process.env.PROTOC ?? 'protoc' });
    const count = generateBindings({ targets, exportCatalog, compile });
    assert.ok(count >= 61);
    assert.ok(existsSync(join(targets.packageTypes, 'latch/v1/agent_pb.ts')));
    assert.ok(existsSync(join(targets.packageTypes, 'latch/v1/index.ts')));
    const protobufTs = readFileSync(join(targets.packageTypes, 'latch/v1/agent_pb.ts'), 'utf8');
    assert.ok(protobufTs.includes('service latch.v1.LatchAgent'));
    assert.ok(protobufTs.includes('watchEvents:'));
    assert.ok(protobufTs.includes('methodKind: "server_streaming"'));
    assert.ok(existsSync(join(targets.contracts, 'EntryPayload.ts')));
    assert.ok(existsSync(join(targets.contracts, 'rpc-methods.ts')));
    assert.ok(existsSync(join(targets.contracts, 'domain-catalog.json')));
    const manifest = JSON.parse(readFileSync(targets.manifest, 'utf8'));
    assert.equal(manifest.procedures.length, count);

    const generatedFile = join(targets.contracts, 'EntryPayload.ts');
    const before = statSync(generatedFile).mtimeMs;
    writeFileSync(generatedFile, 'stale');
    const staleBeforeCheck = readFileSync(generatedFile, 'utf8');
    assert.throws(
      () => generateBindings({ targets, exportCatalog, compile, check: true }),
      /stale/
    );
    assert.equal(readFileSync(generatedFile, 'utf8'), staleBeforeCheck);
    writeFileSync(join(targets.contracts, 'Obsolete.ts'), 'stale');
    generateBindings({ targets, exportCatalog, compile });
    assert.equal(readFileSync(generatedFile, 'utf8').includes('type EntryPayload'), true);
    assert.equal(existsSync(join(targets.contracts, 'Obsolete.ts')), false);
    assert.ok(statSync(generatedFile).mtimeMs >= before);
    assert.equal(generateBindings({ targets, exportCatalog, compile, check: true }), count);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
