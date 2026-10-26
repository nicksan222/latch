import { AgentEventKind, LatchAgent } from '@latch/rpc/v1';
import { cleanup, act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { entryToInput } from '~/lib/entries';
import { entryId, Providers, setupFixture } from '~/test/helpers';
import { setTransport } from './client';
import type { FixtureClient } from './fixture';
import {
  createQueryClient,
  scrubOnLock,
  useAgentEvents,
  useRpcMutation,
  useEntries
} from './queries';

const PLAINTEXT = 'brand-new-plaintext-pw';

let backend: FixtureClient;
beforeEach(async () => {
  backend = await setupFixture();
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
  setTransport(null);
  vi.restoreAllMocks();
});

/** Everything the QueryClient still holds, serialized. */
function cacheDump(client: ReturnType<typeof createQueryClient>): string {
  return JSON.stringify(
    {
      mutations: client
        .getMutationCache()
        .getAll()
        .map((m) => m.state),
      queries: client
        .getQueryCache()
        .getAll()
        .map((q) => q.state.data)
    },
    (_key, value) => (typeof value === 'bigint' ? value.toString() : value)
  );
}

async function updateInput(name: string) {
  const id = await entryId(backend, name);
  const entry = await backend.handle(LatchAgent.method.entriesGet, { id, reveal: false });
  const input = entryToInput(entry);
  if (input.payload?.data.case !== 'login') throw new Error('type');
  input.payload.data.value.password = PLAINTEXT;
  return { id, input };
}

function gateMethod(method: typeof LatchAgent.method.entriesUpdate) {
  let release!: () => void;
  const gate = new Promise<void>((r) => {
    release = r;
  });
  const original = backend.requestBytes.bind(backend);
  vi.spyOn(backend, 'requestBytes').mockImplementation(async (m, p) => {
    if (m === method) await gate;
    return original(m, p);
  });
  return release;
}

describe('useRpcMutation plaintext hygiene', () => {
  it('does not retain variables in the MutationCache or the observer after settling', async () => {
    const client = createQueryClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <Providers client={client}>{children}</Providers>
    );
    const onSuccess = vi.fn();
    const { result } = renderHook(
      () => useRpcMutation(LatchAgent.method.entriesUpdate, { onSuccess }),
      {
        wrapper
      }
    );
    const params = await updateInput('GitHub');
    await act(async () => {
      await result.current.mutateAsync(params);
    });
    expect(onSuccess).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(result.current.variables).toBeUndefined());
    expect(result.current.isIdle).toBe(true);
    await waitFor(() => expect(client.getMutationCache().getAll()).toHaveLength(0));
    expect(cacheDump(client)).not.toContain(PLAINTEXT);
  });

  it('lock clears pending mutations; a late completion neither repopulates nor runs side effects', async () => {
    const client = createQueryClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <Providers client={client}>{children}</Providers>
    );
    const onSuccess = vi.fn();
    const onError = vi.fn();
    const release = gateMethod(LatchAgent.method.entriesUpdate);
    const { result } = renderHook(
      () => useRpcMutation(LatchAgent.method.entriesUpdate, { onSuccess, onError }),
      {
        wrapper
      }
    );
    const params = await updateInput('GitHub');
    act(() => result.current.mutate(params));
    await waitFor(() => expect(client.getMutationCache().getAll()).toHaveLength(1));
    expect(cacheDump(client)).toContain(PLAINTEXT); // in flight: held by the call only

    act(() => scrubOnLock(client));
    expect(client.getMutationCache().getAll()).toHaveLength(0);
    expect(cacheDump(client)).not.toContain(PLAINTEXT);

    await act(async () => {
      release();
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(onSuccess).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
    expect(client.getMutationCache().getAll()).toHaveLength(0);
    expect(cacheDump(client)).not.toContain(PLAINTEXT);
  });

  it('the agent `locked` event clears the MutationCache', async () => {
    const client = createQueryClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <Providers client={client}>{children}</Providers>
    );
    const release = gateMethod(LatchAgent.method.entriesUpdate);
    const { result } = renderHook(
      () => {
        useAgentEvents();
        return useRpcMutation(LatchAgent.method.entriesUpdate);
      },
      { wrapper }
    );
    const params = await updateInput('GitHub');
    act(() => result.current.mutate(params));
    await waitFor(() => expect(client.getMutationCache().getAll()).toHaveLength(1));
    await act(async () => {
      await backend.handle(LatchAgent.method.vaultLock);
      await new Promise((r) => setTimeout(r, 0));
    });
    await waitFor(() => expect(client.getMutationCache().getAll()).toHaveLength(0));
    expect(cacheDump(client)).not.toContain(PLAINTEXT);
    release();
  });
});

it('a settings vault_changed event restarts active queries instead of orphaning them', async () => {
  const client = createQueryClient();
  const event = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Providers client={client}>{children}</Providers>
  );
  const original = backend.requestBytes.bind(backend);
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let started = false;
  vi.spyOn(backend, 'requestBytes').mockImplementation(async (method, params) => {
    if (method === LatchAgent.method.entriesList) {
      started = true;
      await gate;
    }
    return original(method, params);
  });
  const { result } = renderHook(
    () => {
      useAgentEvents(event);
      return useEntries({ profileId: backend.fixture.profiles.acme! });
    },
    { wrapper }
  );
  await waitFor(() => expect(started).toBe(true));
  await act(async () => {
    const settings = await backend.handle(LatchAgent.method.settingsGet);
    await backend.handle(LatchAgent.method.settingsUpdate, settings);
  });
  await waitFor(() =>
    expect(event).toHaveBeenCalledWith(
      expect.objectContaining({ kind: AgentEventKind.VAULT_CHANGED })
    )
  );
  await act(async () => release());
  await waitFor(() => expect(result.current.isSuccess).toBe(true));
  expect(result.current.data?.some((entry) => entry.name === 'GitHub')).toBe(true);
});

it('a lock during invalidation suppresses a late mutation success callback', async () => {
  const client = createQueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Providers client={client}>{children}</Providers>
  );
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const invalidation = vi.spyOn(client, 'invalidateQueries').mockImplementation(async () => gate);
  const onSuccess = vi.fn();
  const { result } = renderHook(
    () => useRpcMutation(LatchAgent.method.entriesUpdate, { invalidate: [['entries']], onSuccess }),
    { wrapper }
  );
  const params = await updateInput('GitHub');
  let completion!: Promise<unknown>;
  act(() => {
    completion = result.current.mutateAsync(params);
  });
  await waitFor(() => expect(invalidation).toHaveBeenCalled());
  act(() => scrubOnLock(client));
  await act(async () => {
    release();
    await completion;
  });
  expect(onSuccess).not.toHaveBeenCalled();
  expect(cacheDump(client)).not.toContain(PLAINTEXT);
});
