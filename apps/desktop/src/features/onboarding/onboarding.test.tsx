import { create, toBinary } from '@bufbuild/protobuf';
import { KdfBenchmarkSchema, LatchAgent } from '@latch/rpc/v1';
import type { KdfBenchmark } from '@latch/rpc/v1';
import { act, fireEvent, screen } from '@testing-library/react';
import { StrictMode } from 'react';
import { afterEach, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { FixtureClient, transportFor } from '~/lib/api/fixture';
import { renderWithProviders } from '~/test/helpers';
import { Onboarding } from './onboarding';

afterEach(() => {
  setTransport(null);
  vi.restoreAllMocks();
});

it('clears calibration progress and ignores a benchmark completed after leaving the step', async () => {
  const backend = new FixtureClient({ unlocked: false });
  await backend.ready;
  const transport = transportFor(backend);
  let complete!: (result: KdfBenchmark) => void;
  const pending = new Promise<KdfBenchmark>((resolve) => {
    complete = resolve;
  });
  const benchmark = vi.fn(() => pending);
  setTransport({
    ...transport,
    request: async (method, payload) =>
      method === LatchAgent.method.vaultBenchmarkKdf
        ? toBinary(KdfBenchmarkSchema, await benchmark())
        : transport.request(method, payload)
  });
  const interval = vi.spyOn(globalThis, 'setInterval');
  const clear = vi.spyOn(globalThis, 'clearInterval');
  const view = renderWithProviders(
    <StrictMode>
      <Onboarding />
    </StrictMode>
  );
  try {
    fireEvent.click(screen.getByRole('button', { name: /Create a new vault/ }));
    fireEvent.change(screen.getByLabelText('Vault folder'), {
      target: { value: '/demo/test-vault' }
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    });
    expect(benchmark).toHaveBeenCalledTimes(1);
    const progressCall = interval.mock.calls.findIndex((args) => args[1] === 80);
    expect(progressCall).toBeGreaterThanOrEqual(0);
    const progressTimer = interval.mock.results[progressCall]!.value;
    fireEvent.click(screen.getByRole('button', { name: /Back/ }));
    expect(clear).toHaveBeenCalledWith(progressTimer);
    await act(async () => {
      complete(
        create(KdfBenchmarkSchema, {
          params: { memoryKib: 65536, iterations: 3, parallelism: 1 },
          measuredMs: 1000
        })
      );
      await pending;
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    });
    // A stale completion must not fill the parent wizard's calibration result.
    expect(benchmark).toHaveBeenCalledTimes(2);
  } finally {
    view.unmount();
    await backend.dispose();
  }
});
