import { spawnSync } from 'node:child_process';
import {
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
import { compileProto, defaultProtoTargets } from './protobuf.mjs';

const repository = resolve(import.meta.dirname, '../..');
const defaultTargets = defaultProtoTargets();

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

function collect(directory) {
  return files(directory).map((path) => ({
    path,
    source: readFileSync(join(directory, path))
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

function stageOutput(target, source) {
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

/** Compile privately first; check mode never writes generated outputs. */
export function generateBindings({
  check = false,
  targets = defaultTargets,
  compile = compileProto,
  exportCatalog = runCatalogExport
} = {}) {
  const temporary = mkdtempSync(join(tmpdir(), 'latch-protocol-'));
  const packageTypes = join(temporary, 'rpc-ts');
  const catalogDirectory = join(temporary, 'catalog');
  mkdirSync(packageTypes, { recursive: true });
  mkdirSync(catalogDirectory, { recursive: true });
  try {
    const compiled = compile({ outputDirectory: packageTypes });
    const generatedTs = join(compiled.tsOutput, 'latch/v1/agent_pb.ts');
    if (!existsSync(generatedTs))
      throw new Error('protoc-gen-es did not emit the Protobuf TypeScript contract');
    writeFileSync(join(compiled.tsOutput, 'latch/v1/index.ts'), 'export * from "./agent_pb";\n');

    exportCatalog({ env: { ...process.env, LATCH_CATALOG_EXPORT_DIR: catalogDirectory } });
    const catalogPath = join(catalogDirectory, 'domain-catalog.json');
    if (!existsSync(catalogPath))
      throw new Error('core domain catalog export did not produce domain-catalog.json');

    const outputs = [
      ...collect(compiled.tsOutput).map(({ path, source }) => ({
        target: join(targets.packageTypes, path),
        source
      })),
      { target: targets.catalog, source: readFileSync(catalogPath) }
    ];
    const differences = outputs
      .filter(
        ({ target, source }) =>
          !existsSync(target) || !readFileSync(target).equals(Buffer.from(source))
      )
      .map(({ target }) => target);
    for (const file of files(targets.packageTypes))
      if (!outputsInDirectory(outputs, targets.packageTypes).includes(file))
        differences.push(join(targets.packageTypes, file));

    if (check && differences.length) {
      throw new Error(
        `Protobuf bindings are stale: ${differences.join(', ')}. Run bun run gen:protocol.`
      );
    }
    if (!check) {
      const expected = outputsInDirectory(outputs, targets.packageTypes);
      for (const file of files(targets.packageTypes))
        if (!expected.includes(file)) rmSync(join(targets.packageTypes, file));
      for (const { target, source } of outputs) stageOutput(target, source);
    }
    return outputs.length;
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

export function defaultOutputTargets() {
  return defaultTargets;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const check = process.argv.includes('--check');
    console.log(
      `Protobuf outputs ${check ? 'verified' : 'generated'} (${generateBindings({ check })} files).`
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
