import type { Transport } from '../transport';
import { FixtureClient, type FixtureOptions } from './client';

export { FixtureClient, type FixtureOptions } from './client';

declare global {
  interface Window {
    /** Dev/e2e handle on the Rust fixture controller (never present in the Tauri app). */
    __latchFixture?: FixtureClient;
  }
}

/** Rust fixture options from the page URL: `?fresh` (no vault), `?unlocked`, `?latency=ms`. */
export function optionsFromLocation(search: string): FixtureOptions {
  const params = new URLSearchParams(search);
  const latency = params.get('latency');
  return {
    fresh: params.has('fresh'),
    unlocked: params.has('unlocked'),
    latencyMs: latency !== null ? Number(latency) || 0 : 120,
    kdfDelayMs: params.has('fast') ? 0 : 900
  };
}

export function transportFor(backend: FixtureClient): Transport {
  return {
    kind: 'rust',
    call: (method, params) => backend.handle(method, params),
    subscribe: (listener) => backend.subscribe(listener)
  };
}

const lifecycleWindows = new WeakSet<Window>();

/** bfcache restores reload to bootstrap a new Rust capability and fresh UI state. */
export function installFixturePageLifecycle(target: Window) {
  if (lifecycleWindows.has(target)) return;
  lifecycleWindows.add(target);
  target.addEventListener('pagehide', () => {
    // Page teardown cannot display a useful error. Explicit teardown awaits the
    // same promise and reports failures; Rust's idle expiry remains a backstop.
    void target.__latchFixture?.dispose().catch(() => {});
  });
  target.addEventListener('pageshow', (event) => {
    if (event.persisted) target.location.reload();
  });
}

export function createFixtureTransport(options?: FixtureOptions): Transport {
  const resolved =
    options ?? (typeof window !== 'undefined' ? optionsFromLocation(window.location.search) : {});
  const backend = new FixtureClient(resolved);
  if (typeof window !== 'undefined') {
    window.__latchFixture = backend;
    installFixturePageLifecycle(window);
  }
  return transportFor(backend);
}
