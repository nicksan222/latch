import { ToastProvider } from '@latch/ui/toast';
import { TooltipProvider } from '@latch/ui/tooltip';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { FixtureClient, transportFor, type FixtureOptions } from '~/lib/api/fixture';
import type { DomainResponse } from '../_generated/contracts';

/** Installs a fresh Rust fixture client as the transport. */
export async function setupFixture(
  options: FixtureOptions = { unlocked: true }
): Promise<FixtureClient> {
  const backend = new FixtureClient(options);
  await backend.ready;
  setTransport(transportFor(backend));
  return backend;
}

export function createTestQueryClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } }
  });
}

export function Providers({ children, client }: { children: ReactNode; client?: QueryClient }) {
  return (
    <QueryClientProvider client={client ?? createTestQueryClient()}>
      <TooltipProvider>
        <ToastProvider>{children}</ToastProvider>
      </TooltipProvider>
    </QueryClientProvider>
  );
}

export function renderWithProviders(ui: ReactElement, client?: QueryClient) {
  const queryClient = client ?? createTestQueryClient();
  return { queryClient, ...render(<Providers client={queryClient}>{ui}</Providers>) };
}

/** Looks up a seeded entry id by name. */
export async function entryId(backend: FixtureClient, name: string): Promise<string> {
  const list = (await backend.handle('entries.list', {
    profileId: null,
    query: null,
    type: null
  })) as { id: string; name: string }[];
  const found = list.find((e) => e.name === name);
  if (!found) throw new Error(`No seeded entry ${name}`);
  return found.id;
}

/** Canned wire response for adapter tests; no domain behavior is implemented here. */
export function setupDomainResponse(response: DomainResponse) {
  const call = vi.fn(async () => response);
  setTransport({ kind: 'stub', call, subscribe: () => () => {} });
  return call;
}
