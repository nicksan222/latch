import { afterEach, expect, it, vi } from 'vite-plus/test';
import { isTauri } from './transport';

afterEach(() => vi.unstubAllGlobals());

it.each([true, false])('uses the official Tauri runtime flag (%s)', (running) => {
  vi.stubGlobal('isTauri', running);
  expect(isTauri()).toBe(running);
});

it('does not treat a private IPC mock as a Tauri runtime', () => {
  vi.stubGlobal('isTauri', false);
  vi.stubGlobal('__TAURI_INTERNALS__', {});
  expect(isTauri()).toBe(false);
});
