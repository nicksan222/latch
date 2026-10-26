import { cleanup } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import type { Entry, EntrySummary, VaultStatus } from '../../../_generated/contracts';
import { FixtureClient } from './client';
import { transportFor } from './index';

let backend: FixtureClient;
beforeEach(async () => {
  backend = new FixtureClient();
  await backend.ready;
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
});
const unlock = () =>
  backend.handle('vault.unlock', {
    passphrase: backend.fixture.passphrase,
    identityFile: null,
    timeoutSecs: null
  });

describe('Rust fixture controller', () => {
  it('exposes Rust-generated metadata and forwards locked RPC errors', async () => {
    expect(transportFor(backend).kind).toBe('rust');
    expect(backend.fixture.profiles.acme).toMatch(/^[a-z2-7]{26}$/);
    expect(await backend.handle('vault.status', null)).toMatchObject({
      initialized: true,
      locked: true
    });
    await expect(backend.handle('profiles.list', null)).rejects.toMatchObject({ code: 'locked' });
  });

  it('uses fixture IDs to fetch a redacted entry, then explicitly reveals', async () => {
    await unlock();
    const id = backend.fixture.entries['acme/GitHub'];
    const list = (await backend.handle('entries.list', {
      profileId: backend.fixture.profiles.acme,
      query: null,
      type: null
    })) as EntrySummary[];
    expect(list.find((entry) => entry.name === 'GitHub')?.id).toBe(id);
    const entry = (await backend.handle('entries.get', { id, reveal: false })) as Entry;
    expect(entry.redactedFields).toContain('password');
    expect(JSON.stringify(entry)).not.toContain('hunter2-acme-Gh!');
    expect(await backend.handle('entries.reveal', { id, field: 'password' })).toEqual({
      value: 'hunter2-acme-Gh!'
    });
  });

  it('inspects the Rust clipboard without recording secret parameters', async () => {
    await unlock();
    const result = await backend.handle('clipboard.copy', {
      id: backend.fixture.entries['acme/GitHub'],
      field: 'password'
    });
    expect(result).toEqual({ clearAfterSecs: 30 });
    expect((await backend.inspect()).clipboard).toBe('hunter2-acme-Gh!');
    expect(backend.clipboard).toBe('hunter2-acme-Gh!');
    expect(backend.calls.every((call) => Object.keys(call).join() === 'method')).toBe(true);
    expect(JSON.stringify(backend.calls)).not.toContain(backend.fixture.passphrase);
  });

  it('forwards invalid credentials without retaining them in diagnostics', async () => {
    await expect(
      backend.handle('vault.unlock', {
        passphrase: 'wrong-fictional-passphrase',
        identityFile: null,
        timeoutSecs: null
      })
    ).rejects.toMatchObject({ code: 'bad_credentials' });
    expect(JSON.stringify(backend.calls)).not.toContain('wrong-fictional-passphrase');
    const status = (await backend.handle('vault.status', null)) as VaultStatus;
    expect(status.locked).toBe(true);
  });

  it('closes its capability and rejects calls after disposal', async () => {
    await backend.dispose();
    await expect(backend.handle('vault.status', null)).rejects.toMatchObject({ code: 'io' });
  });
});
