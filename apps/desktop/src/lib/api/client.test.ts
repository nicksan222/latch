import { cleanup, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, expectTypeOf, it, vi } from 'vite-plus/test';
import type { Entry, EntrySummary, VaultStatus } from '../../_generated/contracts';
import { call, onAgentEvent, selectTransport, setTransport } from './client';
import { RpcError, isRpcError } from './errors';
import { FixtureClient, transportFor } from './fixture';
import { METHODS, type ParamsArg, type ResultOf } from './methods';

afterEach(async () => {
  cleanup();
  await window.__latchFixture?.dispose();
  delete window.__latchFixture;
  vi.unstubAllGlobals();
  setTransport(null);
  vi.unstubAllEnvs();
});

describe('method map', () => {
  it('exposes unique generated methods without retired extension APIs', () => {
    expect(new Set(METHODS).size).toBe(METHODS.length);
    expect(METHODS).not.toContain('browser.installNativeHost');
    expect(METHODS).not.toContain('autofill.fill');
  });

  it('types params and results per method', () => {
    expectTypeOf<ResultOf<'vault.status'>>().toEqualTypeOf<VaultStatus>();
    expectTypeOf<ResultOf<'entries.get'>>().toEqualTypeOf<Entry>();
    expectTypeOf<ResultOf<'entries.list'>>().toEqualTypeOf<EntrySummary[]>();
    expectTypeOf<ParamsArg<'vault.status'>>().toEqualTypeOf<[]>();
    expectTypeOf<ResultOf<'entries.delete'>>().toEqualTypeOf<null>();
    expectTypeOf<ParamsArg<'vault.trust'>>().toEqualTypeOf<[params: Record<string, never>]>();
    expectTypeOf<ParamsArg<'entries.reveal'>>().toEqualTypeOf<
      [params: { id: string; field: string }]
    >();
    // @ts-expect-error -- missing params
    void (() => call('entries.get'));
    // @ts-expect-error -- wrong param shape
    void (() => call('entries.get', { id: 1, reveal: true }));
  });
});

describe('transport selection', () => {
  it('requires desktop IPC in production instead of opening a development session', async () => {
    vi.stubEnv('DEV', false);
    await expect(selectTransport()).rejects.toMatchObject({ code: 'io' });
    expect(window.__latchFixture).toBeUndefined();
  });

  it('uses the Rust HTTP fixture outside Tauri', async () => {
    const t = await selectTransport();
    expect(t.kind).toBe('rust');
    await window.__latchFixture?.ready;
  });

  it('requires app transport injection in Tauri even during development', async () => {
    vi.stubGlobal('isTauri', true);
    await expect(selectTransport()).rejects.toMatchObject({
      code: 'io',
      message: 'Desktop SDK transport has not been configured'
    });
    expect(window.__latchFixture).toBeUndefined();
  });

  it('maps injected transport rejections to typed RpcError', async () => {
    const invoke = vi.fn();
    setTransport({ kind: 'stub', call: invoke, subscribe: () => () => {} });
    invoke.mockRejectedValueOnce({ code: 'locked', message: 'Vault is locked' });
    const err = await call('profiles.list').catch((e: unknown) => e);
    expect(err).toBeInstanceOf(RpcError);
    expect(isRpcError(err, 'locked')).toBe(true);
    expect((err as RpcError).method).toBe('profiles.list');
    invoke.mockRejectedValueOnce(JSON.stringify({ code: 'bad_credentials', message: 'nope' }));
    await expect(
      call('vault.unlock', { passphrase: 'x', identityFile: null, timeoutSecs: null })
    ).rejects.toMatchObject({ code: 'bad_credentials' });
    invoke.mockRejectedValueOnce('plain failure');
    await expect(call('vault.lock')).rejects.toMatchObject({
      code: 'internal',
      message: 'plain failure'
    });
  });
});

describe('events', () => {
  it('forwards backend events to subscribers', async () => {
    const backend = new FixtureClient({ unlocked: true });
    await backend.ready;
    setTransport(transportFor(backend));
    const seen: string[] = [];
    const off = onAgentEvent((e) => seen.push(e.kind));
    await call('vault.lock');
    await waitFor(() => expect(seen).toContain('locked'));
    off();
    await backend.dispose();
  });
});
