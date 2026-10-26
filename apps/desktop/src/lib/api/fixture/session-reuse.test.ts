import { afterEach, expect, it, vi } from 'vite-plus/test';
import { type FixtureClient } from './client';
import { createFixtureTransport } from './index';

let backend: FixtureClient | undefined;
afterEach(async () => {
  await backend?.dispose();
  delete window.__latchFixture;
});

it('reuses real Rust capacity across more page lifecycles than the session cap', async () => {
  // No transport mocking: this test runs against the owned Rust bridge. Fresh
  // sessions avoid vault seeding so the regression exercises allocation/release.
  for (let i = 0; i < 35; i++) {
    createFixtureTransport({ fresh: true });
    backend = window.__latchFixture!;
    await backend.ready;
    const dispose = vi.spyOn(backend, 'dispose');
    window.dispatchEvent(new PageTransitionEvent('pagehide'));
    expect(dispose).toHaveBeenCalledOnce();
    await dispose.mock.results[0]!.value;
    await expect(backend.handle('vault.status', null)).rejects.toMatchObject({ code: 'io' });
    expect(backend.calls).toEqual([]);
  }
}, 60_000);
