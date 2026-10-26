import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  renameSync,
  writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeIndex } from './gen-protocol-index.mjs';
import { compileProto, defaultProtoTargets, generateProtoOutputs } from './protobuf.mjs';
import { generateDesktopSdk } from './sdk.mjs';

const repository = resolve(import.meta.dirname, '../..');
const defaultTargets = defaultProtoTargets();
const catalogTarget = join(repository, 'packages/core/bindings/domain-catalog.json');

function files(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? files(path).map((file) => join(entry.name, file)) : [entry.name];
    })
    .sort();
}

export function compareFiles(actualDirectory, expectedDirectory) {
  const actual = files(actualDirectory);
  const expected = files(expectedDirectory);
  return [...new Set([...actual, ...expected])]
    .sort()
    .filter(
      (file) =>
        !actual.includes(file) ||
        !expected.includes(file) ||
        !readFileSync(join(actualDirectory, file)).equals(
          readFileSync(join(expectedDirectory, file))
        )
    );
}

function runCatalogExport({ env, cwd = repository }) {
  const result = spawnSync(
    'cargo',
    [
      'test',
      '--locked',
      '-p',
      'latch-core',
      '--no-default-features',
      '--lib',
      'export_bindings_domain_catalog'
    ],
    { cwd, env, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }
  );
  if (result.error) throw result.error;
  if (result.status !== 0) {
    process.stderr.write(result.stdout ?? '');
    process.stderr.write(result.stderr ?? '');
    throw new Error(`domain catalog export failed (exit ${result.status})`);
  }
  if (!/test result: ok\. 1 passed/.test(result.stdout)) {
    throw new Error('domain catalog export test did not run');
  }
  return result.stdout;
}

function writeOutputs(directory, outputs) {
  for (const output of outputs) {
    const path = join(directory, output.name);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, output.source);
  }
}

function collect(directory) {
  return files(directory).map((file) => ({
    path: file,
    source: readFileSync(join(directory, file))
  }));
}

function outputsInDirectory(outputs, directory) {
  const root = resolve(directory);
  return outputs.flatMap(({ target }) => {
    const path = resolve(target);
    const rel = relative(root, path);
    if (rel === '..' || rel.startsWith(`..${sep}`) || isAbsolute(rel)) return [];
    return [rel];
  });
}

/** Protobuf is compiled privately first; check mode never writes and failed generation preserves outputs. */
export function generateBindings({
  check = false,
  targets = defaultTargets,
  compile = compileProto,
  exportCatalog = runCatalogExport
} = {}) {
  const temporary = mkdtempSync(join(tmpdir(), 'latch-protocol-'));
  const packageTypes = join(temporary, 'rpc-ts');
  const contracts = join(temporary, 'contracts');
  const catalogDirectory = join(temporary, 'catalog');
  mkdirSync(packageTypes, { recursive: true });
  mkdirSync(contracts, { recursive: true });
  mkdirSync(catalogDirectory, { recursive: true });
  try {
    const compiled = compile({ outputDirectory: packageTypes });
    const { types, manifest } = generateProtoOutputs(compiled.descriptor);
    const protoDirectory = join(packageTypes, 'latch/v1');
    const generatedTs = join(compiled.tsOutput, 'latch/v1/agent_pb.ts');
    if (!existsSync(generatedTs))
      throw new Error('protoc-gen-es did not emit the Protobuf TypeScript contract');
    writeFileSync(join(protoDirectory, 'index.ts'), 'export * from "./agent_pb";\n');
    writeOutputs(contracts, types);
    const desktopSdk = generateDesktopSdk(manifest);
    const manifestText = `${JSON.stringify(manifest, null, 2)}\n`;
    writeFileSync(join(contracts, 'procedures.json'), manifestText);
    exportCatalog({ env: { ...process.env, LATCH_CATALOG_EXPORT_DIR: catalogDirectory } });
    const catalogPath = join(catalogDirectory, 'domain-catalog.json');
    if (!existsSync(catalogPath))
      throw new Error('core domain catalog export did not produce domain-catalog.json');
    copyFileSync(catalogPath, join(contracts, 'domain-catalog.json'));
    writeIndex(contracts);
    const manifestPath = join(contracts, 'procedures.json');
    rmSync(manifestPath);

    const outputs = [
      ...collect(packageTypes).map(({ path, source }) => ({
        target: join(targets.packageTypes, path),
        source
      })),
      ...collect(contracts).map(({ path, source }) => ({
        target: join(targets.contracts, path),
        source
      })),
      { target: targets.desktopSdk, source: desktopSdk },
      { target: targets.manifest, source: manifestText },
      { target: join(targets.contracts, 'domain-catalog.json'), source: readFileSync(catalogPath) }
    ];
    const differences = outputs
      .filter(
        ({ target, source }) =>
          !existsSync(target) || !readFileSync(target).equals(Buffer.from(source))
      )
      .map(({ target }) => target);
    for (const targetDirectory of [targets.packageTypes, targets.contracts]) {
      const actual = files(targetDirectory);
      const expected = outputsInDirectory(outputs, targetDirectory);
      for (const file of actual)
        if (!expected.includes(file)) differences.push(join(targetDirectory, file));
    }
    if (check && differences.length) {
      throw new Error(
        `Protobuf bindings are stale: ${differences.join(', ')}. Run npm run gen:protocol.`
      );
    }
    if (!check) {
      for (const targetDirectory of [targets.packageTypes, targets.contracts]) {
        mkdirSync(targetDirectory, { recursive: true });
        const expected = outputsInDirectory(outputs, targetDirectory);
        for (const file of files(targetDirectory))
          if (!expected.includes(file)) rmSync(join(targetDirectory, file));
      }
      for (const { target, source } of outputs) {
        mkdirSync(dirname(target), { recursive: true });
        const staging = mkdtempSync(join(dirname(target), '.latch-protocol-'));
        try {
          const path = join(staging, 'output');
          writeFileSync(path, source);
          renameSync(path, target);
        } finally {
          rmSync(staging, { recursive: true, force: true });
        }
      }
    }
    return manifest.procedures.length;
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

export function defaultOutputTargets() {
  return { ...defaultTargets, catalog: catalogTarget };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const check = process.argv.includes('--check');
    console.log(
      `Protobuf bindings ${check ? 'verified' : 'generated'} (${generateBindings({ check })} typed procedures).`
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
