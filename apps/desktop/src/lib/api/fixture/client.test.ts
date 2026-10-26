import { ErrorCode, LatchAgent, SyncMode } from '@latch/rpc/v1';
import { cleanup } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
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
  backend.handle(LatchAgent.method.vaultUnlock, {
    passphrase: backend.fixture.passphrase,
    identityFile: undefined,
    timeoutSecs: undefined
  });

describe('Rust fixture controller', () => {
  it('exposes Rust-generated metadata and forwards locked RPC errors', async () => {
    expect(transportFor(backend).kind).toBe('rust');
    expect(backend.fixture.profiles.acme).toMatch(/^[a-z2-7]{26}$/);
    expect(await backend.handle(LatchAgent.method.vaultStatus)).toMatchObject({
      initialized: true,
      locked: true
    });
    await expect(backend.handle(LatchAgent.method.profilesList)).rejects.toMatchObject({
      code: ErrorCode.LOCKED
    });
  });

  it('uses fixture IDs to fetch a redacted entry, then explicitly reveals', async () => {
    await unlock();
    const id = backend.fixture.entries['acme/GitHub'];
    const list = (
      await backend.handle(LatchAgent.method.entriesList, {
        profileId: backend.fixture.profiles.acme,
        query: undefined,
        type: undefined
      })
    ).items;
    expect(list.find((entry) => entry.name === 'GitHub')?.id).toBe(id);
    const entry = await backend.handle(LatchAgent.method.entriesGet, { id, reveal: false });
    expect(entry.redactedFields).toContain('password');
    expect(JSON.stringify(entry)).not.toContain('hunter2-acme-Gh!');
    expect(
      await backend.handle(LatchAgent.method.entriesReveal, { id, field: 'password' })
    ).toMatchObject({
      value: 'hunter2-acme-Gh!'
    });
  });

  it('inspects the Rust clipboard without recording secret parameters', async () => {
    await unlock();
    const result = await backend.handle(LatchAgent.method.clipboardCopy, {
      id: backend.fixture.entries['acme/GitHub'],
      field: 'password'
    });
    expect(result).toMatchObject({ clearAfterSecs: 30 });
    expect((await backend.inspect()).clipboard).toBe('hunter2-acme-Gh!');
    expect(backend.clipboard).toBe('hunter2-acme-Gh!');
    expect(backend.calls.every((call) => Object.keys(call).join() === 'method')).toBe(true);
    expect(JSON.stringify(backend.calls)).not.toContain(backend.fixture.passphrase);
  });

  it('forwards invalid credentials without retaining them in diagnostics', async () => {
    await expect(
      backend.handle(LatchAgent.method.vaultUnlock, {
        passphrase: 'wrong-fictional-passphrase',
        identityFile: undefined,
        timeoutSecs: undefined
      })
    ).rejects.toMatchObject({ code: ErrorCode.BAD_CREDENTIALS });
    expect(JSON.stringify(backend.calls)).not.toContain('wrong-fictional-passphrase');
    const status = await backend.handle(LatchAgent.method.vaultStatus);
    expect(status.locked).toBe(true);
  });

  it('prepares a named sync scenario through the fixture controller', async () => {
    await unlock();
    expect((await backend.handle(LatchAgent.method.syncStatus)).hasRemote).toBe(false);
    await backend.syncScenario('remote_change');
    expect((await backend.handle(LatchAgent.method.syncStatus)).hasRemote).toBe(true);
    const result = await backend.handle(LatchAgent.method.syncRun, { mode: SyncMode.BOTH });
    expect(result.conflicts).toEqual([]);
    expect(
      await backend.handle(LatchAgent.method.entriesReveal, {
        id: backend.fixture.entries['acme/GitHub'],
        field: 'password'
      })
    ).toMatchObject({ value: 'fixture-remote-password' });
  });

  it('closes its capability and rejects calls after disposal', async () => {
    await backend.dispose();
    await expect(backend.handle(LatchAgent.method.vaultStatus)).rejects.toMatchObject({
      code: ErrorCode.IO
    });
  });
});
