import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import { LatchAgent } from '@latch/rpc/v1';
import { QueryClientProvider } from '@tanstack/react-query';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { createQueryClient, scrubOnLock } from '~/lib/api/queries';
import { notify } from '~/lib/notify';
import { useCopyField } from './use-copy-field';

vi.mock('~/lib/notify', () => ({ notify: { success: vi.fn(), error: vi.fn() } }));

afterEach(() => {
  cleanup();
  setTransport(null);
  vi.clearAllMocks();
});

function setup() {
  const client = createQueryClient();
  const call = vi.fn();
  setTransport({
    kind: 'stub',
    request: async (method, bytes) =>
      toBinary(
        method.output,
        create(method.output, await call(method, fromBinary(method.input, bytes)))
      ),
    subscribe: () => () => {}
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, call, ...renderHook(useCopyField, { wrapper }) };
}

const args = { id: 'entry', field: 'password', label: 'Password' };
const response = { clearAfterSecs: 30 };

describe('useCopyField', () => {
  it('keeps labels local, delivers per-call callbacks, and scrubs a mounted observer', async () => {
    const { client, call, result } = setup();
    call.mockResolvedValue(response);
    const onSuccess = vi.fn();
    const onSettled = vi.fn();
    await act(async () => {
      await result.current.mutateAsync(args, { onSuccess, onSettled });
    });
    expect(call).toHaveBeenCalledExactlyOnceWith(
      LatchAgent.method.clipboardCopy,
      expect.objectContaining({
        id: args.id,
        field: args.field
      })
    );
    expect(notify.success).toHaveBeenCalledExactlyOnceWith(
      'Copied Password',
      'Clipboard clears in 30s'
    );
    expect(onSuccess).toHaveBeenCalledWith(
      expect.objectContaining(response),
      args,
      expect.objectContaining({ epoch: expect.any(Number) }),
      expect.any(Object)
    );
    expect(onSettled).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(result.current.isIdle).toBe(true));
    expect(result.current.variables).toBeUndefined();
    expect(result.current.data).toBeUndefined();
    expect(client.getMutationCache().getAll()).toHaveLength(0);
  });

  it.each(['success', 'error'] as const)(
    'suppresses a pending copy %s toast after lock',
    async (outcome) => {
      const { client, call, result } = setup();
      let resolve!: (value: typeof response) => void;
      let reject!: (error: Error) => void;
      call.mockReturnValue(
        new Promise((done, fail) => {
          resolve = done;
          reject = fail;
        })
      );
      let completion!: Promise<unknown>;
      act(() => {
        completion = result.current.mutateAsync(args).catch(() => undefined);
      });
      await waitFor(() => expect(call).toHaveBeenCalled());
      act(() => scrubOnLock(client));
      await act(async () => {
        if (outcome === 'success') resolve(response);
        else reject(new Error('Copy failed'));
        await completion;
      });
      expect(notify.success).not.toHaveBeenCalled();
      expect(notify.error).not.toHaveBeenCalled();
      await waitFor(() => expect(result.current.variables).toBeUndefined());
      expect(client.getMutationCache().getAll()).toHaveLength(0);
    }
  );

  it('delivers error labels and per-call error callbacks', async () => {
    const { call, result } = setup();
    call.mockRejectedValue(new Error('Copy failed'));
    const onError = vi.fn();
    await act(async () => {
      await result.current.mutateAsync(args, { onError }).catch(() => undefined);
    });
    expect(notify.error).toHaveBeenCalledWith('Could not copy Password', expect.any(Error));
    expect(onError).toHaveBeenCalledWith(
      expect.any(Error),
      args,
      expect.objectContaining({ epoch: expect.any(Number) }),
      expect.any(Object)
    );
    await waitFor(() => expect(result.current.variables).toBeUndefined());
  });

  it('still reports success after palette-style immediate unmount', async () => {
    const { call, result, unmount } = setup();
    let resolve!: (value: typeof response) => void;
    call.mockReturnValue(
      new Promise((done) => {
        resolve = done;
      })
    );
    let completion!: Promise<unknown>;
    act(() => {
      completion = result.current.mutateAsync(args);
    });
    unmount();
    await waitFor(() => expect(call).toHaveBeenCalled());
    await act(async () => {
      resolve(response);
      await completion;
    });
    expect(notify.success).toHaveBeenCalledWith('Copied Password', 'Clipboard clears in 30s');
  });
});
