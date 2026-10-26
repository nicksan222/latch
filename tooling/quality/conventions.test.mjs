import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { checkConventions } from './conventions.mjs';

function fixture(files, run) {
  const root = mkdtempSync(join(tmpdir(), 'latch-conventions-'));
  try {
    for (const [name, value] of Object.entries(files)) {
      mkdirSync(dirname(join(root, name)), { recursive: true });
      writeFileSync(join(root, name), typeof value === 'string' ? value : JSON.stringify(value));
    }
    return run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
const manifest = (name, rest = {}) => ({ name, private: true, ...rest });
const base = {
  'package.json': manifest('latch-workspace', {
    workspaces: ['apps/*', 'packages/*', 'tooling/*']
  }),
  'packages/ui/package.json': manifest('@latch/ui', { exports: { './*': './src/*.tsx' } }),
  'packages/ui/src/button.tsx': 'export const Button = 1;'
};

test('public wildcard and conditional exports resolve without exposing implementation paths', () => {
  fixture(
    {
      ...base,
      'tooling/check/package.json': manifest('@latch/check', {
        exports: { '.': { types: './types.d.ts', default: './index.mjs' } }
      }),
      'tooling/check/types.d.ts': '',
      'tooling/check/index.mjs': ''
    },
    (root) => assert.deepEqual(checkConventions(root), [])
  );
});

test('package boundaries reject a shared UI dependency on an app', () => {
  fixture(
    {
      ...base,
      'apps/desktop/package.json': manifest('@latch/desktop'),
      'packages/ui/package.json': manifest('@latch/ui', { dependencies: { '@latch/desktop': '*' } })
    },
    (root) => assert.ok(checkConventions(root).some((s) => /packages\/ui.*depends on app/.test(s)))
  );
});

test('missing, escaping and absolute public export targets fail', () => {
  for (const target of ['./missing.ts', '../escape.ts', '/absolute.ts']) {
    fixture(
      { ...base, 'packages/ui/package.json': manifest('@latch/ui', { exports: target }) },
      (root) => assert.ok(checkConventions(root).some((s) => s.includes('export target')))
    );
  }
});

test('duplicate package identities and missing local dependencies fail', () => {
  fixture(
    {
      ...base,
      'apps/desktop/package.json': manifest('@latch/ui', {
        dependencies: { '@latch/missing': '*' }
      })
    },
    (root) => {
      const errors = checkConventions(root);
      assert.ok(errors.some((s) => s.includes('duplicate workspace name')));
      assert.ok(errors.some((s) => s.includes('@latch/missing')));
    }
  );
});

test('legacy root implementation directories are rejected', () => {
  fixture({ ...base, 'scripts/old.mjs': '' }, (root) =>
    assert.ok(checkConventions(root).some((s) => s.includes('scripts/')))
  );
});

test('the concurrently owned landing workspace is outside this quality gate', () => {
  fixture({ ...base, 'apps/landing/package.json': 'in-progress landing manifest' }, (root) =>
    assert.deepEqual(checkConventions(root), [])
  );
});

test('handwritten TypeScript filenames use kebab-case, including paired tests', () => {
  fixture(
    { ...base, 'packages/ui/src/SecretField.tsx': '', 'packages/ui/src/useCopyField.ts': '' },
    (root) => {
      const errors = checkConventions(root);
      assert.ok(errors.some((s) => s.includes('SecretField.tsx') && s.includes('kebab-case')));
      assert.ok(errors.some((s) => s.includes('useCopyField.ts') && s.includes('kebab-case')));
    }
  );
  fixture(
    {
      ...base,
      'packages/ui/src/secret-field.test.tsx': '',
      'apps/desktop/src/_generated/contracts/EntryType.ts': ''
    },
    (root) => assert.deepEqual(checkConventions(root), [])
  );
});

test('apps contain only the CLI, desktop and landing deployable units', () => {
  fixture({ ...base, 'apps/web/package.json': manifest('@latch/web') }, (root) => {
    assert.ok(
      checkConventions(root).some((error) =>
        error.includes('apps/web: only deployable CLI, desktop and landing units belong in apps')
      )
    );
  });
});

test('Rust feature implementations live behind directory module facades', () => {
  fixture({ ...base, 'packages/core/src/core/secrets.rs': 'pub fn save() {}' }, (root) => {
    assert.ok(checkConventions(root).some((error) => error.includes('directory module')));
  });
  fixture({ ...base, 'packages/core/src/core/secrets/mod.rs': 'pub fn save() {}' }, (root) =>
    assert.deepEqual(checkConventions(root), [])
  );
});

test('product Git processes go through the shared Git module', () => {
  fixture({ ...base, 'apps/cli/src/bypass.rs': 'std::process::Command::new("git");' }, (root) =>
    assert.ok(checkConventions(root).some((error) => error.includes('shared Git')))
  );
  fixture({ ...base, 'packages/core/src/core/git/process.rs': 'Command::new("git");' }, (root) =>
    assert.deepEqual(checkConventions(root), [])
  );
  fixture(
    { ...base, 'packages/core/src/core/vault/tests/acceptance.rs': 'Command::new("git");' },
    (root) => assert.deepEqual(checkConventions(root), [])
  );
});

test('acceptance implementations are owned by modules or apps, not generic tooling', () => {
  fixture({ ...base, 'tooling/e2e/secrets.spec.ts': '' }, (root) => {
    assert.ok(checkConventions(root).some((error) => error.includes('app-owned')));
  });
  fixture(
    { ...base, 'packages/core/tests/secrets.rs': '#[test] fn creates_secret() {}' },
    (root) => {
      assert.ok(checkConventions(root).some((error) => error.includes('module-owned')));
    }
  );
  fixture(
    {
      ...base,
      'apps/desktop/tests/e2e/secrets.spec.ts': '',
      'packages/core/tests/secrets.rs': 'include!("../src/core/secrets/tests/acceptance.rs");'
    },
    (root) => assert.deepEqual(checkConventions(root), [])
  );
});

test('CLI action implementations keep their tests below the action', () => {
  const action = 'pub(super) fn save() {}';
  fixture({ ...base, 'apps/cli/src/cli/commands/save.rs': action }, (root) => {
    assert.ok(checkConventions(root).some((error) => error.includes('colocated action tests')));
  });
  fixture(
    {
      ...base,
      'apps/cli/src/cli/commands/save.rs': `${action}\n#[cfg(test)]\nmod tests { #[test] fn rejects_invalid_input() {} }`,
      'apps/cli/src/cli/commands/mod.rs': 'mod save;'
    },
    (root) => assert.deepEqual(checkConventions(root), [])
  );
});

test('contracts belong to consumers, never a shared protocol workspace', () => {
  fixture({ ...base, 'packages/protocol/package.json': manifest('@latch/protocol') }, (root) => {
    assert.ok(
      checkConventions(root).some((error) =>
        error.includes('packages/rpc as the single authoritative interface')
      )
    );
  });
  fixture(
    { ...base, 'apps/desktop/src/client.ts': "import type { Entry } from '@latch/protocol';" },
    (root) => {
      assert.ok(
        checkConventions(root).some((error) => error.includes('authoritative generated interface'))
      );
    }
  );
});

test('the interface crate cannot depend on the core implementation', () => {
  fixture({ ...base, 'packages/rpc/Cargo.toml': '[dependencies]\nprost = "0.14"\n' }, (root) =>
    assert.deepEqual(checkConventions(root), [])
  );
  for (const dependency of [
    'latch-core = { path = "../core" }',
    'latch = { package = "latch-core", path = "../core" }'
  ]) {
    fixture({ ...base, 'packages/rpc/Cargo.toml': `[dependencies]\n${dependency}\n` }, (root) =>
      assert.ok(
        checkConventions(root).some((error) =>
          error.includes('interface crate must not depend on latch-core')
        )
      )
    );
  }
});

test('Cargo manifests cannot add ts-rs directly or through an alias', () => {
  for (const manifestText of [
    '[dependencies]\nts-rs = "10"\n',
    '[dependencies]\nserde-types = { package = "ts-rs", version = "10" }\n',
    '[dependencies.serde-types]\nversion = "10"\npackage = "ts-rs"\n',
    '[workspace.dependencies.ts-rs]\nversion = "10"\n'
  ]) {
    fixture({ ...base, 'packages/example/Cargo.toml': manifestText }, (root) =>
      assert.ok(checkConventions(root).some((error) => error.includes('ts-rs is forbidden')))
    );
  }
  fixture({ ...base, 'packages/example/Cargo.toml': '[dependencies]\nserde = "1"\n' }, (root) =>
    assert.deepEqual(checkConventions(root), [])
  );
});

test('external RPC scenarios cannot bypass the raw generated gRPC client', () => {
  const dir = 'packages/core/src/rpc/tests/external';
  fixture(
    {
      ...base,
      [`${dir}/transport.rs`]: 'use latch::rpc::{AgentClient, RpcSdk};',
      [`${dir}/host.rs`]: 'use latch::agent::Agent;'
    },
    (root) => {
      const errors = checkConventions(root);
      assert.ok(errors.some((error) => error.includes('external RPC scenarios')));
      assert.ok(!errors.some((error) => error.includes(`${dir}/host.rs`)));
    }
  );
  fixture(
    {
      ...base,
      [`${dir}/transport.rs`]: 'use latch_rpc::v1::latch_agent_client::LatchAgentClient;'
    },
    (root) => assert.deepEqual(checkConventions(root), [])
  );
});

test('transitive dependencies cannot reintroduce the retired generator', () => {
  fixture({ ...base, 'Cargo.lock': '[[package]]\nname = "ts-rs"\nversion = "10.0.0"\n' }, (root) =>
    assert.ok(checkConventions(root).some((error) => error.includes('Cargo.lock')))
  );
});

test('external clients cannot import backend protocol aliases', () => {
  for (const source of ['use latch::protocol::VaultStatus;', 'use latch as implementation;']) {
    fixture({ ...base, 'packages/core/src/rpc/tests/external/client.rs': source }, (root) =>
      assert.ok(checkConventions(root).some((error) => error.includes('external RPC scenarios')))
    );
  }
});
