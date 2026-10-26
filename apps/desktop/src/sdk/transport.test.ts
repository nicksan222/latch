import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import type { AgentEvent } from '../_generated/contracts';
import { createTauriTransport } from './transport';

const { invoke, listen } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
});

// Dynamic imports and listen resolve asynchronously, including after unsubscribe.
async function flushRegistration() {
  await vi.waitFor(() => expect(listen).toHaveBeenCalled());
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

function emit(payload: AgentEvent) {
  const callback = listen.mock.calls.at(-1)![1] as EventCallback<AgentEvent>;
  callback({ event: 'latch://event', id: 1, payload });
}

describe('Tauri transport', () => {
  it('invokes rpc with parameters and preserves the result', async () => {
    const transport = createTauriTransport();
    const result = { value: 'revealed' };
    invoke.mockResolvedValueOnce(result);
    await expect(transport.call('entries.reveal', { id: 'a', field: 'password' })).resolves.toBe(
      result
    );
    expect(invoke).toHaveBeenCalledExactlyOnceWith('rpc', {
      method: 'entries.reveal',
      params: { id: 'a', field: 'password' }
    });
  });

  it.each([undefined, null])('passes absent parameters as null (%s)', async (params) => {
    await createTauriTransport().call('vault.status', params);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('rpc', { method: 'vault.status', params: null });
  });

  it.each([{ code: 'locked', message: 'Vault is locked' }, 'native failure'])(
    'preserves invoke rejections (%s)',
    async (error) => {
      invoke.mockRejectedValueOnce(error);
      await expect(createTauriTransport().call('profiles.list', null)).rejects.toBe(error);
    }
  );

  it('shares one native listener and disposes it when the final subscriber leaves', async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValueOnce(unlisten);
    const transport = createTauriTransport();
    const first = vi.fn();
    const second = vi.fn();
    const offFirst = transport.subscribe(first);
    const offSecond = transport.subscribe(second);
    await flushRegistration();
    expect(listen).toHaveBeenCalledExactlyOnceWith('latch://event', expect.any(Function));
    const payload: AgentEvent = { kind: 'locked' };
    emit(payload);
    expect(first).toHaveBeenCalledExactlyOnceWith(payload);
    expect(second).toHaveBeenCalledExactlyOnceWith(payload);
    offFirst();
    expect(unlisten).not.toHaveBeenCalled();
    emit(payload);
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(2);
    offSecond();
    offSecond();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('disposes a native handle that resolves after unsubscribe', async () => {
    const unlisten = vi.fn();
    let resolve!: (unlisten: UnlistenFn) => void;
    listen.mockReturnValueOnce(new Promise<UnlistenFn>((done) => (resolve = done)));
    const subscriber = vi.fn();
    const off = createTauriTransport().subscribe(subscriber);
    off();
    await flushRegistration();
    emit({ kind: 'locked' });
    expect(subscriber).not.toHaveBeenCalled();
    resolve(unlisten);
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('reuses pending registration on resubscription and can register again after disposal', async () => {
    const unlisten = vi.fn();
    const nextUnlisten = vi.fn();
    let resolve!: (unlisten: UnlistenFn) => void;
    listen
      .mockReturnValueOnce(new Promise<UnlistenFn>((done) => (resolve = done)))
      .mockResolvedValueOnce(nextUnlisten);
    const transport = createTauriTransport();
    const offFirst = transport.subscribe(vi.fn());
    await flushRegistration();
    offFirst();
    const second = vi.fn();
    const offSecond = transport.subscribe(second);
    resolve(unlisten);
    await flushRegistration();
    expect(listen).toHaveBeenCalledTimes(1);
    expect(unlisten).not.toHaveBeenCalled();
    emit({ kind: 'locked' });
    expect(second).toHaveBeenCalledTimes(1);
    offSecond();
    expect(unlisten).toHaveBeenCalledTimes(1);
    const offThird = transport.subscribe(vi.fn());
    await vi.waitFor(() => expect(listen).toHaveBeenCalledTimes(2));
    await flushRegistration();
    offThird();
    expect(nextUnlisten).toHaveBeenCalledTimes(1);
  });

  it('can subscribe again after native registration fails', async () => {
    const unlisten = vi.fn();
    listen.mockRejectedValueOnce(new Error('registration failed')).mockResolvedValueOnce(unlisten);
    const transport = createTauriTransport();
    const offFirst = transport.subscribe(vi.fn());
    await flushRegistration();
    offFirst();
    const subscriber = vi.fn();
    const offSecond = transport.subscribe(subscriber);
    await vi.waitFor(() => expect(listen).toHaveBeenCalledTimes(2));
    await flushRegistration();
    emit({ kind: 'locked' });
    expect(subscriber).toHaveBeenCalledExactlyOnceWith({ kind: 'locked' });
    offSecond();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
