import assert from 'node:assert/strict';
import test from 'node:test';
import { evaluate, nodeSupported, toolchainRequirements } from './doctor.mjs';

const requirements = toolchainRequirements(
  '[toolchain]\nchannel = "1.98.1"\ncomponents = ["clippy", "rustfmt", "rust-src"]'
);
const healthy = {
  node: 'v24.2.0',
  platform: 'linux',
  tools: { npm: '11', git: '2', rustup: '1', cargo: '1', rustc: 'rustc 1.98.1 (fixture)' },
  rustVersion: 'rustc 1.98.1 (fixture)',
  components: ['clippy-x86_64-unknown-linux-gnu', 'rustfmt-x86_64-unknown-linux-gnu', 'rust-src'],
  desktopLibraries: true,
  desktopTools: true,
  novnc: true
};

test('Node minimum rejects old or malformed versions and accepts newer majors', () => {
  for (const version of ['v22.1.0', 'unknown', '240']) assert.equal(nodeSupported(version), false);
  for (const version of ['v24.0.0', '25.1.2']) assert.equal(nodeSupported(version), true);
});

test('requirements come from the pin, including multiline components', () => {
  assert.deepEqual(
    toolchainRequirements(
      '[toolchain]\nchannel = "1.99.2"\ncomponents = [\n "rustfmt",\n "clippy",\n]'
    ),
    { channel: '1.99.2', components: ['rustfmt', 'clippy'] }
  );
  assert.throws(() => toolchainRequirements('[toolchain]'), /declare/);
});

test('healthy fixture passes all checks', () => {
  assert.ok(evaluate(healthy, requirements).every((check) => check.ok));
});

test('missing tools, wrong Rust and missing components have actionable failures', () => {
  const failed = evaluate(
    { ...healthy, tools: {}, rustVersion: 'rustc 1.97.0', components: ['clippy-preview'] },
    requirements
  ).filter((check) => !check.ok);
  assert.ok(failed.some((check) => check.name === 'git'));
  assert.ok(failed.some((check) => check.name === 'Rust 1.98.1'));
  assert.ok(failed.some((check) => check.name === 'Rust component rustfmt'));
  assert.ok(failed.every((check) => check.fix.length > 10));
});

test('desktop readiness is optional and Linux-specific', () => {
  const stopped = evaluate(
    { ...healthy, desktopLibraries: false, desktopTools: false, novnc: false },
    requirements
  );
  assert.equal(stopped.filter((check) => !check.ok && check.optional).length, 3);
  assert.ok(
    evaluate({ ...healthy, platform: 'darwin', novnc: false }, requirements).every(
      (check) => check.ok
    )
  );
});
