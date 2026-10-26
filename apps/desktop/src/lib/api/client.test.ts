import { type MessageInitShape } from '@bufbuild/protobuf';
import { AgentEventKind, type EntriesListResponse, ErrorCode } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { Entry, VaultStatus, EmptySchema, RevealParamsSchema } from '@latch/rpc/v1';
import { cleanup, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, expectTypeOf, it, vi } from 'vite-plus/test';
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
    expect(new Set(METHODS.map((m) => m.name)).size).toBe(METHODS.length);
    expect(METHODS.map((m) => m.name)).not.toContain('BrowserInstallNativeHost');
    expect(METHODS.map((m) => m.name)).not.toContain('AutofillFill');
  });

  it('types params and results per method', () => {
    expectTypeOf<ResultOf<typeof LatchAgent.method.vaultStatus>>().toEqualTypeOf<VaultStatus>();
    expectTypeOf<ResultOf<typeof LatchAgent.method.entriesGet>>().toEqualTypeOf<Entry>();
    expectTypeOf<
      ResultOf<typeof LatchAgent.method.entriesList>
    >().toEqualTypeOf<EntriesListResponse>();
    expectTypeOf<ParamsArg<typeof LatchAgent.method.vaultStatus>>().toEqualTypeOf<
      [params?: MessageInitShape<typeof EmptySchema>]
    >();
    expectTypeOf<ResultOf<typeof LatchAgent.method.entriesDelete>>().toEqualTypeOf<
      import('@latch/rpc/v1').Empty
    >();
    expectTypeOf<ParamsArg<typeof LatchAgent.method.vaultTrust>>().toEqualTypeOf<
      [params?: MessageInitShape<typeof EmptySchema>]
    >();
    expectTypeOf<ParamsArg<typeof LatchAgent.method.entriesReveal>>().toEqualTypeOf<
      [params: MessageInitShape<typeof RevealParamsSchema>]
    >();
    // @ts-expect-error -- missing params
    void (() => call(LatchAgent.method.entriesGet));
    // @ts-expect-error -- wrong param shape
    void (() => call(LatchAgent.method.entriesGet, { id: 1, reveal: true }));
  });
});

describe('transport selection', () => {
  it('requires desktop IPC in production instead of opening a development session', async () => {
    vi.stubEnv('DEV', false);
    await expect(selectTransport()).rejects.toMatchObject({ code: ErrorCode.IO });
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
      code: ErrorCode.IO,
      message: 'Desktop SDK transport has not been configured'
    });
    expect(window.__latchFixture).toBeUndefined();
  });

  it('maps injected transport rejections to typed RpcError', async () => {
    const invoke = vi.fn();
    setTransport({ kind: 'stub', request: invoke, subscribe: () => () => {} });
    invoke.mockRejectedValueOnce({ code: ErrorCode.LOCKED, message: 'Vault is locked' });
    const err = await call(LatchAgent.method.profilesList).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(RpcError);
    expect(isRpcError(err, ErrorCode.LOCKED)).toBe(true);
    expect((err as RpcError).method).toBe(LatchAgent.method.profilesList.name);
    invoke.mockRejectedValueOnce(
      JSON.stringify({ code: ErrorCode.BAD_CREDENTIALS, message: 'nope' })
    );
    await expect(
      call(LatchAgent.method.vaultUnlock, {
        passphrase: 'x',
        identityFile: undefined,
        timeoutSecs: undefined
      })
    ).rejects.toMatchObject({ code: ErrorCode.BAD_CREDENTIALS });
    invoke.mockRejectedValueOnce('plain failure');
    await expect(call(LatchAgent.method.vaultLock)).rejects.toMatchObject({
      code: ErrorCode.INTERNAL,
      message: 'plain failure'
    });
  });
});

describe('events', () => {
  it('forwards backend events to subscribers', async () => {
    const backend = new FixtureClient({ unlocked: true });
    await backend.ready;
    setTransport(transportFor(backend));
    const seen: AgentEventKind[] = [];
    const off = onAgentEvent((e) => seen.push(e.kind));
    await call(LatchAgent.method.vaultLock);
    await waitFor(() => expect(seen).toContain(AgentEventKind.LOCKED));
    off();
    await backend.dispose();
  });
});
