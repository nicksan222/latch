import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const protoRoot = new URL('../../packages/rpc/proto/', import.meta.url);
const protoPath = new URL('../../packages/rpc/proto/latch/v1/agent.proto', import.meta.url);
const generatedPackage = new URL('../../packages/rpc/ts/', import.meta.url);
const catalogTarget = new URL(
  '../../apps/desktop/src/_generated/domain-catalog.json',
  import.meta.url
);
const plugin = new URL('../../node_modules/.bin/protoc-gen-es', import.meta.url);
const repository = localPath(new URL('../../', import.meta.url));

function localPath(url) {
  return fileURLToPath(url);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} failed (${result.status}): ${result.stderr || result.stdout}`);
  }
  return result.stdout;
}

export function compileProto({
  proto = localPath(protoPath),
  protoDirectory = localPath(protoRoot),
  pluginPath = localPath(plugin),
  protoc,
  outputDirectory
} = {}) {
  const tsOutput = outputDirectory ?? mkdtempSync(join(tmpdir(), 'latch-proto-ts-'));
  mkdirSync(tsOutput, { recursive: true });
  const protocArgs = [
    `--proto_path=${protoDirectory}`,
    `--plugin=protoc-gen-es=${pluginPath}`,
    `--es_out=${tsOutput}`,
    '--es_opt=target=ts',
    proto
  ];
  if (protoc ?? process.env.PROTOC) {
    run(protoc ?? process.env.PROTOC, protocArgs, { cwd: repository });
  } else {
    run(
      'cargo',
      ['run', '--locked', '-p', 'latch-rpc', '--example', 'protoc', '--', ...protocArgs],
      { cwd: repository }
    );
  }
  return { tsOutput };
}

export function defaultProtoTargets() {
  return {
    packageTypes: localPath(generatedPackage),
    catalog: localPath(catalogTarget)
  };
}
