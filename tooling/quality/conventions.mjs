import { existsSync, globSync, readdirSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

function inside(root, target) {
  const path = relative(root, target);
  return path !== '..' && !path.startsWith(`..${sep}`) && !isAbsolute(path);
}

function targets(exports) {
  if (typeof exports === 'string') return [exports];
  if (!exports || typeof exports !== 'object') return [];
  return Object.values(exports).flatMap(targets);
}

/** Cross-file manifest rules; syntax/import rules remain owned by TypeScript and Oxlint. */
export function checkConventions(root) {
  const errors = [];
  const read = (path) => JSON.parse(readFileSync(join(root, path), 'utf8'));
  const rootManifest = read('package.json');
  const workspaces = rootManifest.workspaces;
  if (!Array.isArray(workspaces)) return ['package.json: workspaces must be an array'];
  const manifests = [];
  for (const pattern of workspaces) {
    if (
      typeof pattern !== 'string' ||
      !/^(apps|packages|tooling)\//.test(pattern) ||
      pattern.split('/').includes('..')
    ) {
      errors.push(`package.json: workspace outside apps/packages/tooling: ${pattern}`);
      continue;
    }
    for (const path of globSync(`${pattern}/package.json`, { cwd: root })) {
      // Explicitly outside this round: the landing thread owns its own validation.
      if (path.replaceAll('\\', '/') === 'apps/landing/package.json') continue;
      manifests.push({ path, manifest: read(path) });
    }
  }
  // Cargo must have one interface generation source: the Protobuf schema.
  for (const path of globSync('**/Cargo.toml', {
    cwd: root,
    dot: true,
    exclude: ['**/target/**', '**/node_modules/**']
  })) {
    const cargo = readFileSync(join(root, path), 'utf8');
    if (
      /^\s*ts-rs\s*=|\bpackage\s*=\s*["']ts-rs["']|^\s*\[[^\]]*(?:dependencies|workspace\.dependencies)\.ts-rs\]/m.test(
        cargo
      )
    ) {
      errors.push(`${path}: ts-rs is forbidden; Protobuf is the sole contract generator`);
    }
  }
  if (existsSync(join(root, 'apps'))) {
    for (const entry of readdirSync(join(root, 'apps'), { withFileTypes: true })) {
      if (entry.isDirectory() && !['cli', 'desktop', 'landing'].includes(entry.name)) {
        errors.push(
          `apps/${entry.name}: only deployable CLI, desktop and landing units belong in apps`
        );
      }
    }
  }
  if (existsSync(join(root, 'packages/protocol'))) {
    errors.push('packages/protocol: use packages/rpc as the single authoritative interface');
  }
  const lockPath = join(root, 'Cargo.lock');
  if (existsSync(lockPath) && /^name\s*=\s*["']ts-rs["']/m.test(readFileSync(lockPath, 'utf8'))) {
    errors.push('Cargo.lock: ts-rs is forbidden, including transitive dependencies');
  }
  const rpcManifest = join(root, 'packages/rpc/Cargo.toml');
  if (existsSync(rpcManifest)) {
    const cargo = readFileSync(rpcManifest, 'utf8');
    if (
      /^\s*(?:latch-core\s*=|[^\n=]+\s*=\s*\{[^\n]*package\s*=\s*["']latch-core["'])/m.test(cargo)
    ) {
      errors.push('packages/rpc/Cargo.toml: interface crate must not depend on latch-core');
    }
  }
  const names = new Map();
  for (const { path, manifest } of manifests) {
    if (names.has(manifest.name)) errors.push(`${path}: duplicate workspace name ${manifest.name}`);
    names.set(manifest.name, path);
    if (!manifest.name?.startsWith('@latch/'))
      errors.push(`${path}: expected @latch/ workspace name`);
    if (manifest.private !== true) errors.push(`${path}: workspace must be private`);
  }
  for (const { path, manifest } of manifests) {
    const packageRoot = resolve(root, dirname(path));
    for (const target of targets(manifest.exports)) {
      if (!target.startsWith('./') || !inside(packageRoot, resolve(packageRoot, target))) {
        errors.push(`${path}: escaping export target ${target}`);
        continue;
      }
      const files = globSync(target, { cwd: packageRoot }).filter((name) =>
        statSync(join(packageRoot, name)).isFile()
      );
      if (
        !files.length ||
        files.some(
          (name) => !inside(realpathSync(packageRoot), realpathSync(join(packageRoot, name)))
        )
      ) {
        errors.push(`${path}: missing or escaping export target ${target}`);
      }
    }
    const dependencies = {
      ...manifest.dependencies,
      ...manifest.devDependencies,
      ...manifest.peerDependencies,
      ...manifest.optionalDependencies
    };
    for (const dependency of Object.keys(dependencies).filter((name) =>
      name.startsWith('@latch/')
    )) {
      const destination = names.get(dependency);
      if (!destination) errors.push(`${path}: unknown local dependency ${dependency}`);
      else if (destination.startsWith(`apps${sep}`) && path !== destination) {
        errors.push(`${path}: depends on app ${dependency}; consume a shared package instead`);
      }
    }
  }
  // Generated interface bindings are not handwritten application contracts.
  const handwritten = [
    'apps/desktop/src/**/*.{ts,tsx}',
    'packages/ui/src/**/*.{ts,tsx}',
    'tooling/**/*.{ts,tsx}'
  ];
  for (const pattern of handwritten) {
    for (const path of globSync(pattern, {
      cwd: root,
      exclude: ['**/node_modules/**', '**/dist/**', '**/gen/**', '**/_generated/**']
    })) {
      if (/['"]@latch\/protocol(?:['"]|\/)/.test(readFileSync(join(root, path), 'utf8'))) {
        errors.push(`${path}: use the authoritative generated interface from @latch/rpc`);
      }
      const name = path.split(/[\\/]/).at(-1);
      if (!/^[a-z0-9]+(?:-[a-z0-9]+)*(?:\.[a-z0-9]+(?:-[a-z0-9]+)*)*\.(?:ts|tsx)$/.test(name)) {
        errors.push(`${path}: handwritten TypeScript filenames must use kebab-case`);
      }
    }
  }
  for (const legacy of ['scripts', 'e2e', 'fixtures', 'src-tauri']) {
    if (existsSync(join(root, legacy)))
      errors.push(`${legacy}/: implementation belongs in apps/packages/tooling`);
  }
  for (const path of globSync('packages/core/src/core/*.rs', { cwd: root })) {
    if (!['mod.rs', 'error.rs', 'ids.rs', 'paths.rs'].includes(path.split(/[\\/]/).at(-1))) {
      errors.push(`${path}: feature implementation belongs behind a directory module facade`);
    }
  }
  for (const pattern of ['packages/core/src/**/*.rs', 'apps/{cli,desktop}/**/src/**/*.rs']) {
    for (const path of globSync(pattern, {
      cwd: root,
      exclude: ['**/target/**', '**/gen/**', '**/_generated/**']
    })) {
      const normalized = path.replaceAll('\\', '/');
      if (normalized.startsWith('packages/core/src/core/git/') || normalized.includes('/tests/'))
        continue;
      if (/\bCommand::new\(\s*"git"\s*\)/.test(readFileSync(join(root, path), 'utf8'))) {
        errors.push(`${path}: product Git processes must use the shared Git module`);
      }
    }
  }
  for (const path of globSync('tooling/**/*.spec.{ts,tsx}', {
    cwd: root,
    exclude: ['**/node_modules/**']
  })) {
    errors.push(`${path}: product E2E scenarios must be app-owned`);
  }
  for (const path of globSync('packages/core/tests/*.rs', { cwd: root })) {
    if (/#\[\s*(?:tokio::)?test\b|\bproptest!/.test(readFileSync(join(root, path), 'utf8'))) {
      errors.push(
        `${path}: core acceptance implementations must be module-owned; keep only Cargo discovery here`
      );
    }
  }
  const externalRpc = 'packages/core/src/rpc/tests/external';
  for (const path of globSync(`${externalRpc}/*.rs`, { cwd: root })) {
    if (path === `${externalRpc}/host.rs`) continue;
    const source = readFileSync(join(root, path), 'utf8');
    if (
      /\b(?:AgentClient|LocalClient|RpcSdk|RpcTransport)\b|\blatch(?:_core)?::|\buse\s+latch(?:_core)?\b|\bextern\s+crate\s+latch(?:_core)?\b|\blatch_rpc::(?:client|codec|sdk)\b|\.dispatch\s*\(/.test(
        source
      )
    ) {
      errors.push(
        `${path}: external RPC scenarios must use only the generated latch-rpc gRPC client`
      );
    }
  }
  for (const path of globSync('apps/cli/src/cli/commands/**/*.rs', { cwd: root })) {
    if (['mod.rs', 'support.rs', 'test_support.rs'].includes(path.split(/[\\/]/).at(-1))) continue;
    const source = readFileSync(join(root, path), 'utf8');
    if (!/#\[cfg\(test\)\]\s*mod tests\b/.test(source)) {
      errors.push(`${path}: missing colocated action tests below the implementation`);
    }
  }
  return errors.sort();
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const errors = checkConventions(resolve(import.meta.dirname, '../..'));
  if (errors.length) {
    console.error(errors.join('\n'));
    process.exitCode = 1;
  } else console.log('Workspace names, dependencies, exports and ownership verified.');
}
