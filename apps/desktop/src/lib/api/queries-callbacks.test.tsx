import { QueryClientProvider } from '@tanstack/react-query';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, expect, it, vi } from 'vite-plus/test';
import { setTransport } from './client';
import { createQueryClient, useRpcMutation } from './queries';

afterEach(() => {
  cleanup();
  setTransport(null);
});

it.each(['success', 'error'] as const)(
  'uses rerendered callbacks for both concurrent pending mutation %s completions',
  async (outcome) => {
    const client = createQueryClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const pending: { resolve: (value: unknown) => void; reject: (error: Error) => void }[] = [];
    const call = vi.fn(
      () =>
        new Promise((resolve, reject) => {
          pending.push({ resolve, reject });
        })
    );
    setTransport({ kind: 'stub', call, subscribe: () => () => {} });
    const original = vi.fn();
    const updated = vi.fn();
    const { result, rerender } = renderHook(
      ({ callback }) =>
        useRpcMutation('clipboard.copy', { onSuccess: callback, onError: callback }),
      { wrapper, initialProps: { callback: original } }
    );
    const completions: Promise<unknown>[] = [];
    act(() => {
      completions.push(
        result.current.mutateAsync({ id: 'first', field: 'password' }).catch(() => undefined)
      );
      completions.push(
        result.current.mutateAsync({ id: 'second', field: 'password' }).catch(() => undefined)
      );
    });
    await waitFor(() => expect(pending).toHaveLength(2));
    rerender({ callback: updated });
    await act(async () => {
      for (const request of pending) {
        if (outcome === 'success') request.resolve({ clearAfterSecs: 30 });
        else request.reject(new Error('Copy failed'));
      }
      await Promise.all(completions);
    });
    expect(original).not.toHaveBeenCalled();
    expect(updated).toHaveBeenCalledTimes(2);
  }
);
