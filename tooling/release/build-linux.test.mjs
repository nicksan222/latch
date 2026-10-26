import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { validateArtifactNames } from './publish-release.mjs';

function packagingFixture(missingBundle = false) {
  const root = mkdtempSync(join(tmpdir(), 'latch-package-test-'));
  const tools = join(root, 'fake-bin');
  mkdirSync(join(root, 'tooling/release'), { recursive: true });
  mkdirSync(tools);
  writeFileSync(join(root, 'LICENSE'), 'Fixture license\n');
  for (const name of ['build-linux.sh', 'release-plan.mjs']) {
    cpSync(join(import.meta.dirname, name), join(root, 'tooling/release', name));
  }
  writeFileSync(join(tools, 'rustc'), '#!/bin/sh\necho "host: x86_64-unknown-linux-gnu"\n', {
    mode: 0o755
  });
  writeFileSync(
    join(tools, 'bun'),
    `#!/bin/sh
set -eu
[ "$*" = "run desktop:bundle -- --bundles deb,appimage -- --locked" ]
mkdir -p apps/desktop/src-tauri/binaries "$CARGO_TARGET_DIR/release/bundle/deb" "$CARGO_TARGET_DIR/release/bundle/appimage"
printf '#!/bin/sh\\necho latch 0.1.1\\n' > apps/desktop/src-tauri/binaries/latch
chmod +x apps/desktop/src-tauri/binaries/latch
printf 'test deb' > "$CARGO_TARGET_DIR/release/bundle/deb/Latch_0.1.1_amd64.deb"
${missingBundle ? ':' : 'printf "test appimage" > "$CARGO_TARGET_DIR/release/bundle/appimage/Latch_0.1.1_amd64.AppImage"'}
`,
    { mode: 0o755 }
  );
  return {
    root,
    env: {
      ...process.env,
      PATH: `${tools}:${process.env.PATH}`,
      CI: 'true',
      LATCH_RELEASE_BUILD: '1',
      CARGO_TARGET_DIR: join(root, 'target')
    }
  };
}

test('Linux packager produces exactly four assets and CLI and license archive in a private fixture', () => {
  const { root, env } = packagingFixture();
  try {
    execFileSync('bash', ['tooling/release/build-linux.sh', '0.1.1'], {
      cwd: root,
      env,
      stdio: 'pipe'
    });
    const assets = readdirSync(join(root, 'release-artifacts'));
    validateArtifactNames(assets);
    const archive = assets.find((name) => name.endsWith('.tar.gz'));
    assert.equal(
      execFileSync('tar', ['-tzf', join(root, 'release-artifacts', archive)], { encoding: 'utf8' }),
      'latch\nLICENSE\n'
    );
    execFileSync('sha256sum', ['--check', 'SHA256SUMS'], {
      cwd: join(root, 'release-artifacts'),
      stdio: 'pipe'
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('Linux packager fails closed when a desktop bundle is missing', () => {
  const { root, env } = packagingFixture(true);
  try {
    assert.throws(
      () =>
        execFileSync('bash', ['tooling/release/build-linux.sh', '0.1.1'], {
          cwd: root,
          env,
          stdio: 'pipe'
        }),
      /Expected one appimage bundle/
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('desktop bundle configuration builds desktop and CLI without removed applications', () => {
  const config = JSON.parse(
    readFileSync(
      new URL('../../apps/desktop/src-tauri/tauri.bundle.conf.json', import.meta.url),
      'utf8'
    )
  );
  assert.equal(
    config.build.beforeBuildCommand,
    'bun run --filter @latch/desktop build && sh ../../tooling/release/build-sidecars.sh'
  );
  assert.equal(config.bundle.externalBin, undefined);
  assert.deepEqual(config.bundle.resources, { '../../../LICENSE': 'LICENSE' });
  assert.equal(config.bundle.linux.deb.files['/usr/share/doc/latch/copyright'], '../../../LICENSE');
  assert.equal(config.bundle.linux.deb.files['/usr/bin/latch'], 'binaries/latch');
});

test('CLI packaging helper honors relative target directories from a desktop working directory', () => {
  const { root, env } = packagingFixture();
  try {
    cpSync(
      join(import.meta.dirname, 'build-sidecars.sh'),
      join(root, 'tooling/release/build-sidecars.sh')
    );
    mkdirSync(join(root, 'apps/desktop'), { recursive: true });
    writeFileSync(
      join(root, 'fake-bin/cargo'),
      `#!/bin/sh
set -eu
case " $* " in *' -p latch-cli '*) ;; *) exit 2 ;; esac
mkdir -p "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release"
printf 'fixture CLI' > "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/latch"
`,
      { mode: 0o755 }
    );
    execFileSync('sh', [join(root, 'tooling/release/build-sidecars.sh')], {
      cwd: join(root, 'apps/desktop'),
      env: { ...env, CARGO_TARGET_DIR: 'build-cache', TAURI_ENV_DEBUG: 'false' },
      stdio: 'pipe'
    });
    assert.equal(
      readFileSync(join(root, 'apps/desktop/src-tauri/binaries/latch'), 'utf8'),
      'fixture CLI'
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
