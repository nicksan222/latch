import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));

/** Feature ownership is independent of the adapter used to execute a scenario. */
export function validateFeatures(features, exists = (path) => existsSync(resolve(root, path))) {
  const errors = [];
  if (!Array.isArray(features) || !features.length) return ['Feature inventory is empty'];
  const ids = new Set();
  for (const feature of features) {
    if (!/^[a-z][a-z0-9-]*$/.test(feature.id)) errors.push(`Invalid feature: ${feature.id}`);
    if (ids.has(feature.id)) errors.push(`Duplicate feature: ${feature.id}`);
    ids.add(feature.id);
    if (!feature.sources?.length) errors.push(`${feature.id}: missing source ownership`);
    for (const path of [
      ...(feature.sources ?? []),
      ...(feature.acceptance ?? []),
      ...(feature.browser ?? [])
    ]) {
      if (
        !/^(?:apps|packages|tooling)\//.test(path) ||
        path.split('/').includes('..') ||
        !exists(path)
      )
        errors.push(`Missing or invalid feature path: ${path}`);
    }
    if (!feature.rust?.length) errors.push(`${feature.id}: missing Rust acceptance coverage`);
    for (const suite of feature.rust ?? []) {
      if (
        !/^latch-[a-z-]+$/.test(suite.package) ||
        (suite.kind !== 'lib' && !/^[a-z][a-z0-9_]*$/.test(suite.target))
      )
        errors.push(`${feature.id}: invalid Rust test target`);
    }
  }
  return errors;
}

export function featureCommands(features, selected) {
  if (!selected.length) throw new Error('Select at least one feature');
  const commands = new Map();
  for (const id of new Set(selected)) {
    const feature = features.find((item) => item.id === id);
    if (!feature) throw new Error(`Unknown feature: ${id}`);
    for (const suite of feature.rust) {
      const args = [
        'test',
        '--locked',
        '--no-default-features',
        '-p',
        suite.package,
        ...(suite.kind === 'lib' ? ['--lib'] : ['--test', suite.target])
      ];
      if (suite.filter) args.push(suite.filter);
      commands.set(JSON.stringify(args), args);
    }
  }
  return [...commands.values()];
}

function run(args) {
  const features = JSON.parse(readFileSync(new URL('./features.json', import.meta.url), 'utf8'));
  const errors = validateFeatures(features);
  if (errors.length) throw new Error(errors.join('\n'));
  if (args.length === 1 && args[0] === '--check') {
    console.log(`Verified ${features.length} feature owners and coverage paths.`);
    return 0;
  }
  if (args.length === 1 && args[0] === '--list') {
    for (const feature of features) console.log(feature.id);
    return 0;
  }
  const commands = featureCommands(features, args.length ? args : features.map((item) => item.id));
  for (const command of commands) {
    // Cargo exits successfully when a filter matches nothing. Refuse that false green.
    const listed = spawnSync('cargo', [...command, '--', '--list'], {
      cwd: root,
      encoding: 'utf8'
    });
    if (listed.error) throw listed.error;
    if (listed.status !== 0) {
      process.stderr.write(listed.stderr ?? '');
      return listed.status ?? 1;
    }
    if (!/^.+: test$/m.test(listed.stdout))
      throw new Error(`No tests discovered: cargo ${command.join(' ')}`);
    console.log(`cargo ${command.join(' ')}`);
    const result = spawnSync('cargo', command, { cwd: root, stdio: 'inherit' });
    if (result.error) throw result.error;
    if (result.status !== 0) return result.status ?? 1;
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = run(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
