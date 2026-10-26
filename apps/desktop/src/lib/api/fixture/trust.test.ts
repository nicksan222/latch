import { cleanup } from '@testing-library/react';
import { afterEach, beforeEach, expect, it } from 'vite-plus/test';
import type { VaultStatus } from '../../../_generated/contracts';
import { FixtureClient } from './client';

let backend: FixtureClient;
beforeEach(async () => {
  backend = new FixtureClient();
  await backend.ready;
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
});

it('returns the real Rust trust and integrity status', async () => {
  const status = (await backend.handle('vault.status', null)) as VaultStatus;
  expect(status.trust?.fingerprint).toMatch(/^[A-Z0-9]{4}(-[A-Z0-9]{4}){7}$/);
  expect(status.integrityProblems).toEqual([]);
  expect(status.rotationPending).toBe(false);
  expect(await backend.handle('vault.trust', {})).toMatchObject({
    fingerprint: status.trust?.fingerprint
  });
});

it('forwards the Rust timeout validation error', async () => {
  await expect(
    backend.handle('vault.unlock', {
      passphrase: backend.fixture.passphrase,
      identityFile: null,
      timeoutSecs: 0
    })
  ).rejects.toMatchObject({ code: 'invalid_params' });
});
