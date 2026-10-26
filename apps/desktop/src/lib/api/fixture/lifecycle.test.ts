import { waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';
import { FixtureClient } from './client';
import { installFixturePageLifecycle } from './index';

const session = {
  sessionId: 'fictional-lifecycle-capability',
  fixture: { passphrase: '', profiles: {}, entries: {}, vaults: {} },
  inspection: { clipboard: null, lastExport: null }
};
const allocated = () => Response.json(session);
const released = () => new Response(null, { status: 204 });
function pageFor(backend: FixtureClient) {
  const page = Object.assign(new EventTarget(), {
    __latchFixture: backend,
    location: { reload: vi.fn() }
  }) as unknown as Window;
  installFixturePageLifecycle(page);
  return page;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('Rust fixture page lifecycle transport', () => {
  it('releases once with a keepalive body capability after pagehide and repeated dispose', async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(allocated())
      .mockResolvedValueOnce(released());
    vi.stubGlobal('fetch', fetcher);
    const backend = new FixtureClient({ baseUrl: 'http://127.0.0.1:1430' });
    await backend.ready;
    const page = pageFor(backend);
    installFixturePageLifecycle(page);
    page.dispatchEvent(new PageTransitionEvent('pagehide'));
    page.dispatchEvent(new PageTransitionEvent('pagehide'));
    const disposal = backend.dispose();
    expect(backend.dispose()).toBe(disposal);
    await disposal;
    expect(fetcher).toHaveBeenCalledTimes(2);
    const [url, options] = fetcher.mock.calls[1]!;
    expect(url).toBe('http://127.0.0.1:1430/__latch/session');
    expect(options).toMatchObject({
      method: 'DELETE',
      keepalive: true,
      cache: 'no-store',
      credentials: 'omit',
      headers: { Origin: 'http://127.0.0.1:1420', 'Content-Type': 'application/json' },
      body: JSON.stringify({ sessionId: session.sessionId })
    });
    expect(options?.signal).toBeUndefined();
    await expect(backend.handle('vault.status', null)).rejects.toMatchObject({ code: 'io' });
  });

  it('waits for allocation after early pagehide without polling or reopening the session', async () => {
    let resolveAllocation!: (response: Response) => void;
    const allocation = new Promise<Response>((resolve) => {
      resolveAllocation = resolve;
    });
    const fetcher = vi
      .fn<typeof fetch>()
      .mockReturnValueOnce(allocation)
      .mockResolvedValueOnce(released());
    vi.stubGlobal('fetch', fetcher);
    const backend = new FixtureClient();
    backend.subscribe(() => {});
    const page = pageFor(backend);
    page.dispatchEvent(new PageTransitionEvent('pagehide'));
    const disposal = backend.dispose();
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher.mock.calls[0]?.[1]?.signal).toBeUndefined();
    expect(fetcher.mock.calls[0]?.[1]?.keepalive).toBe(true);
    resolveAllocation(allocated());
    await disposal;
    backend.subscribe(() => {});
    await Promise.resolve();
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls[1]?.[1]?.method).toBe('DELETE');
    expect(() => backend.fixture).toThrow(
      'Await client.ready before reading Rust fixture metadata'
    );
  });

  it('releases cached pages and reloads only when restored from bfcache', async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(allocated())
      .mockResolvedValueOnce(released());
    vi.stubGlobal('fetch', fetcher);
    const backend = new FixtureClient();
    await backend.ready;
    const page = pageFor(backend);
    page.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    await backend.dispose();
    page.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: false }));
    expect(page.location.reload).not.toHaveBeenCalled();
    page.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    expect(page.location.reload).toHaveBeenCalledOnce();
  });

  it('aborts an existing event poll and never restarts it after pagehide', async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(allocated())
      .mockImplementationOnce(
        (_url, options) =>
          new Promise((_resolve, reject) => {
            options?.signal?.addEventListener('abort', () =>
              reject(new DOMException('Aborted', 'AbortError'))
            );
          })
      )
      .mockResolvedValueOnce(released());
    vi.stubGlobal('fetch', fetcher);
    const backend = new FixtureClient();
    await backend.ready;
    backend.subscribe(() => {});
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));
    const pollSignal = fetcher.mock.calls[1]?.[1]?.signal;
    pageFor(backend).dispatchEvent(new PageTransitionEvent('pagehide'));
    await backend.dispose();
    expect(pollSignal?.aborted).toBe(true);
    expect(fetcher).toHaveBeenCalledTimes(3);
  });

  it('does not send a release when allocation failed', async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(new Response(null, { status: 503 }));
    vi.stubGlobal('fetch', fetcher);
    const backend = new FixtureClient();
    pageFor(backend).dispatchEvent(new PageTransitionEvent('pagehide'));
    await backend.dispose();
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('does not hide release failures from awaited explicit teardown', async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(allocated())
      .mockResolvedValueOnce(new Response(null, { status: 500 }));
    vi.stubGlobal('fetch', fetcher);
    const backend = new FixtureClient();
    await backend.ready;
    pageFor(backend).dispatchEvent(new PageTransitionEvent('pagehide'));
    await expect(backend.dispose()).rejects.toMatchObject({
      code: 'io',
      message: 'Rust session release failed (500)'
    });
    expect(fetcher).toHaveBeenCalledTimes(2);
  });
});
