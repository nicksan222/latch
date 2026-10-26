import { ErrorCode, LatchAgent } from '@latch/rpc/v1';
import { cleanup } from '@testing-library/react';
import { afterEach, beforeEach, expect, it } from 'vite-plus/test';
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
  const status = await backend.handle(LatchAgent.method.vaultStatus);
  expect(status.trust?.fingerprint).toMatch(/^[A-Z0-9]{4}(-[A-Z0-9]{4}){7}$/);
  expect(status.integrityProblems).toEqual([]);
  expect(status.rotationPending).toBe(false);
  expect(await backend.handle(LatchAgent.method.vaultTrust, {})).toMatchObject({
    fingerprint: status.trust?.fingerprint
  });
});

it('forwards the Rust timeout validation error', async () => {
  await expect(
    backend.handle(LatchAgent.method.vaultUnlock, {
      passphrase: backend.fixture.passphrase,
      identityFile: undefined,
      timeoutSecs: 0n
    })
  ).rejects.toMatchObject({ code: ErrorCode.INVALID_PARAMS });
});
