import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseVersion } from './release-plan.mjs';

const paths = [
  'package.json',
  'bun.lock',
  'Cargo.toml',
  'Cargo.lock',
  'apps/desktop/src-tauri/tauri.conf.json'
];
export function prepareDocuments(documents, version) {
  parseVersion(version);
  const result = { ...documents };
  for (const path of ['package.json', 'apps/desktop/src-tauri/tauri.conf.json']) {
    const value = JSON.parse(documents[path]);
    parseVersion(value.version);
    value.version = version;
    result[path] = JSON.stringify(value, null, 2) + '\n';
  }
  if (!/^\{\n\s+"lockfileVersion": 2,/m.test(documents['bun.lock']))
    throw new Error('Expected Bun text lockfile version 2');
  let workspaceCount = 0;
  result['Cargo.toml'] = documents['Cargo.toml'].replace(
    /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)("\s*\n)/g,
    (_, before, previous, after) => {
      parseVersion(previous);
      workspaceCount++;
      return before + version + after;
    }
  );
  if (workspaceCount !== 1) throw new Error('Expected one workspace package version');
  let packageCount = 0;
  result['Cargo.lock'] = documents['Cargo.lock'].replace(
    /(\[\[package\]\]\nname = "(?:latch-core|latch-cli|latch-web-bridge|latch-desktop|latch-rpc)"\nversion = ")([^"]+)("\n)/g,
    (_, before, previous, after) => {
      parseVersion(previous);
      packageCount++;
      return before + version + after;
    }
  );
  if (packageCount < 1) throw new Error('Missing workspace lock packages');
  return result;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.env.LATCH_RELEASE_BUILD !== '1' || process.env.CI !== 'true')
    throw new Error('Version preparation is restricted to disposable CI build checkouts');
  const documents = Object.fromEntries(paths.map((path) => [path, readFileSync(path, 'utf8')]));
  const prepared = prepareDocuments(documents, process.argv[2]);
  for (const path of paths) writeFileSync(path, prepared[path]);
}
