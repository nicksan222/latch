import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { test } from 'node:test';
import { compareFiles, defaultOutputTargets, generateBindings } from './generate.mjs';

test('default outputs are official RPC TypeScript and the desktop domain catalog only', () => {
  const targets = defaultOutputTargets();
  assert.match(targets.packageTypes, /packages\/rpc\/ts\/?$/);
  assert.match(targets.catalog, /apps\/desktop\/src\/_generated\/domain-catalog\.json$/);
  assert.equal('desktopSdk' in targets, false);
  assert.equal('manifest' in targets, false);
});

test('freshness detects changed, missing and stale files without rewriting them', () => {
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

test('generation atomically writes protoc output and catalog, and checks freshness', () => {
  const root = mkdtempSync(join(tmpdir(), 'latch-proto-generate-'));
  try {
    const targets = {
      packageTypes: `${join(root, 'rpc/ts')}${sep}`,
      catalog: join(root, 'desktop/_generated/domain-catalog.json')
    };
    const exportCatalog = ({ env }) => {
      mkdirSync(env.LATCH_CATALOG_EXPORT_DIR, { recursive: true });
      writeFileSync(
        join(env.LATCH_CATALOG_EXPORT_DIR, 'domain-catalog.json'),
        '{"entryTypes":[]}\n'
      );
    };
    const compile = ({ outputDirectory }) => {
      const output = join(outputDirectory, 'latch/v1');
      mkdirSync(output, { recursive: true });
      writeFileSync(join(output, 'agent_pb.ts'), 'export const official = true;\n');
      return { tsOutput: outputDirectory };
    };

    const count = generateBindings({ targets, exportCatalog, compile });
    assert.equal(count, 3);
    assert.equal(
      readFileSync(join(targets.packageTypes, 'latch/v1/agent_pb.ts'), 'utf8'),
      'export const official = true;\n'
    );
    assert.equal(
      readFileSync(join(targets.packageTypes, 'latch/v1/index.ts'), 'utf8'),
      'export * from "./agent_pb";\n'
    );
    assert.equal(readFileSync(targets.catalog, 'utf8'), '{"entryTypes":[]}\n');

    writeFileSync(join(targets.packageTypes, 'latch/v1/Obsolete.ts'), 'stale');
    writeFileSync(targets.catalog, 'stale catalog');
    const agentPb = join(targets.packageTypes, 'latch/v1/agent_pb.ts');
    const staleBeforeCheck = readFileSync(agentPb, 'utf8');
    assert.throws(
      () => generateBindings({ targets, exportCatalog, compile, check: true }),
      /stale/
    );
    assert.equal(readFileSync(agentPb, 'utf8'), staleBeforeCheck);
    assert.equal(readFileSync(targets.catalog, 'utf8'), 'stale catalog');
    generateBindings({ targets, exportCatalog, compile });
    assert.equal(existsSync(join(targets.packageTypes, 'latch/v1/Obsolete.ts')), false);
    assert.equal(generateBindings({ targets, exportCatalog, compile, check: true }), count);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('failed catalog export leaves existing generated outputs untouched', () => {
  const root = mkdtempSync(join(tmpdir(), 'latch-proto-atomic-'));
  try {
    const targets = {
      packageTypes: join(root, 'rpc/ts'),
      catalog: join(root, 'desktop/domain-catalog.json')
    };
    mkdirSync(join(targets.packageTypes, 'latch/v1'), { recursive: true });
    mkdirSync(join(root, 'desktop'), { recursive: true });
    writeFileSync(join(targets.packageTypes, 'latch/v1/agent_pb.ts'), 'old');
    writeFileSync(targets.catalog, 'old catalog');
    const compile = ({ outputDirectory }) => {
      mkdirSync(join(outputDirectory, 'latch/v1'), { recursive: true });
      writeFileSync(join(outputDirectory, 'latch/v1/agent_pb.ts'), 'new');
      return { tsOutput: outputDirectory };
    };
    assert.throws(
      () =>
        generateBindings({
          targets,
          compile,
          exportCatalog: () => {
            throw new Error('stub failure');
          }
        }),
      /stub failure/
    );
    assert.equal(readFileSync(join(targets.packageTypes, 'latch/v1/agent_pb.ts'), 'utf8'), 'old');
    assert.equal(readFileSync(targets.catalog, 'utf8'), 'old catalog');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
