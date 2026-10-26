import { beforeEach, describe, expect, expectTypeOf, it, vi } from 'vite-plus/test';
import type { Entry, EntryInput, VaultStatus } from '../_generated/contracts';
import { createDesktopSdk } from './generated';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

beforeEach(() => invoke.mockReset());

const entryInput: EntryInput = {
  profileId: 'profile',
  name: 'Note',
  tags: [],
  notes: '',
  favorite: false,
  payload: { type: 'note', data: { body: 'body' } }
};

describe('generated desktop SDK', () => {
  it('calls the official Tauri rpc command without parameters for status', async () => {
    const result: VaultStatus = {
      initialized: false,
      locked: true,
      vaultId: null,
      path: null,
      recipients: [],
      dirty: false,
      ahead: 0,
      behind: 0,
      hasRemote: false,
      lastSyncAt: null,
      locksInSecs: null,
      trust: null,
      integrityProblems: [],
      rotationPending: false,
      memoryLocked: null
    };
    invoke.mockResolvedValueOnce(result);
    const sdk = createDesktopSdk();
    await expect(sdk.vault.status()).resolves.toBe(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('rpc', { method: 'vault.status', params: null });
  });

  it('passes named procedure parameters and preserves its result', async () => {
    const result: Entry = {
      ...entryInput,
      id: 'entry',
      createdAt: '2026-10-25T00:00:00Z',
      updatedAt: '2026-10-25T00:00:00Z',
      redactedFields: []
    };
    invoke.mockResolvedValueOnce(result);
    await expect(createDesktopSdk().entries.create(entryInput)).resolves.toBe(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('rpc', {
      method: 'entries.create',
      params: entryInput
    });
  });

  it('supports an injected official invoke implementation', async () => {
    const customInvoke = vi.fn().mockResolvedValueOnce([]);
    await expect(createDesktopSdk(customInvoke).profiles.list()).resolves.toEqual([]);
    expect(customInvoke).toHaveBeenCalledExactlyOnceWith('rpc', {
      method: 'profiles.list',
      params: null
    });
    expect(invoke).not.toHaveBeenCalled();
  });

  it('dispatches the shared UI boundary through generated procedures', async () => {
    invoke.mockResolvedValueOnce({ value: 'revealed' });
    await expect(
      createDesktopSdk().call('entries.reveal', { id: 'entry', field: 'password' })
    ).resolves.toEqual({ value: 'revealed' });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('rpc', {
      method: 'entries.reveal',
      params: { id: 'entry', field: 'password' }
    });
  });

  it.each(['unknown.method', 'toString', '__proto__'])(
    'rejects unknown procedure %s before IPC',
    async (method) => {
      await expect(createDesktopSdk().call(method, null)).rejects.toBeDefined();
      expect(invoke).not.toHaveBeenCalled();
    }
  );

  it.each([{ code: 'locked', message: 'Vault is locked' }, 'native failure'])(
    'preserves native errors for shared UI normalization (%s)',
    async (error) => {
      invoke.mockRejectedValueOnce(error);
      await expect(createDesktopSdk().vault.status()).rejects.toBe(error);
    }
  );

  it('types procedure requests and responses from the Rust contract', () => {
    const sdk = createDesktopSdk();
    expectTypeOf(sdk.vault.status).returns.toEqualTypeOf<Promise<VaultStatus>>();
    expectTypeOf(sdk.entries.create).parameters.toEqualTypeOf<[params: EntryInput]>();
    expectTypeOf(sdk.entries.create).returns.toEqualTypeOf<Promise<Entry>>();
    // @ts-expect-error -- procedure requires a request
    void (() => sdk.entries.create());
    // @ts-expect-error -- incorrect request shape
    void (() => sdk.entries.create({ profileId: 1 }));
    // @ts-expect-error -- parameterless procedure rejects a request
    void (() => sdk.vault.status({}));
    // @ts-expect-error -- nonexistent procedure
    void (() => sdk.vault.nonexistent());
  });
});
