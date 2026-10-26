import { create, toBinary } from '@bufbuild/protobuf';
import { AgentEventSchema, AgentEventKind, ErrorCode } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { AgentEvent } from '@latch/rpc/v1';
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';
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
  const callback = listen.mock.calls.at(-1)![1] as EventCallback<number[]>;
  callback({
    event: 'latch://event',
    id: 1,
    payload: Array.from(toBinary(AgentEventSchema, payload))
  });
}

describe('Tauri transport', () => {
  it('invokes grpc_request with binary parameters and preserves response bytes', async () => {
    const transport = createTauriTransport();
    const method = LatchAgent.method.entriesReveal;
    const payload = toBinary(method.input, create(method.input, { id: 'a', field: 'password' }));
    const response = toBinary(method.output, create(method.output, { value: 'revealed' }));
    invoke.mockResolvedValueOnce(Array.from(response));
    await expect(transport.request(method, payload)).resolves.toEqual(response);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('grpc_request', {
      method: method.name,
      payload: Array.from(payload)
    });
  });

  it.each([{}, undefined])('encodes empty parameters as empty bytes (%s)', async (params) => {
    invoke.mockResolvedValueOnce([]);
    const method = LatchAgent.method.vaultStatus;
    await createTauriTransport().request(
      method,
      toBinary(method.input, create(method.input, params))
    );
    expect(invoke).toHaveBeenCalledExactlyOnceWith('grpc_request', {
      method: method.name,
      payload: []
    });
  });

  it.each([{ code: ErrorCode.LOCKED, message: 'Vault is locked' }, 'native failure'])(
    'preserves invoke rejections (%s)',
    async (error) => {
      invoke.mockRejectedValueOnce(error);
      await expect(
        createTauriTransport().request(LatchAgent.method.profilesList, new Uint8Array())
      ).rejects.toBe(error);
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
    const payload = create(AgentEventSchema, { kind: AgentEventKind.LOCKED });
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
    emit(create(AgentEventSchema, { kind: AgentEventKind.LOCKED }));
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
    emit(create(AgentEventSchema, { kind: AgentEventKind.LOCKED }));
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
    emit(create(AgentEventSchema, { kind: AgentEventKind.LOCKED }));
    expect(subscriber).toHaveBeenCalledExactlyOnceWith(
      create(AgentEventSchema, { kind: AgentEventKind.LOCKED })
    );
    offSecond();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
