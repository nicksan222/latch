import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { useUiStore } from '~/stores/ui';
import { SHORTCUT_EVENT, handleShortcut, installDesktopEvents } from './desktop-events';

type Handler = (event: { payload: unknown }) => void;
const handlers = new Map<string, Handler>();
const unlisten = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (name: string, handler: Handler) => {
    handlers.set(name, handler);
    return Promise.resolve(unlisten);
  }
}));

describe('desktop events', () => {
  beforeEach(() => {
    handlers.clear();
    unlisten.mockClear();
    useUiStore.getState().setPaletteOpen(false);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('is a no-op outside Tauri', () => {
    const cleanup = installDesktopEvents({ disableContextMenu: true });
    const ev = new MouseEvent('contextmenu', { cancelable: true });
    window.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(false);
    cleanup();
    expect(handlers.size).toBe(0);
  });

  it('opens the palette on the shortcut event', async () => {
    vi.stubGlobal('isTauri', true);
    const cleanup = installDesktopEvents({ disableContextMenu: false });
    await vi.waitFor(() => expect(handlers.has(SHORTCUT_EVENT)).toBe(true));
    handlers.get(SHORTCUT_EVENT)!({ payload: { action: 'palette' } });
    expect(useUiStore.getState().paletteOpen).toBe(true);
    cleanup();
    expect(unlisten).toHaveBeenCalled();
  });

  it('ignores unknown shortcut actions', () => {
    handleShortcut({ action: 'other' } as never);
    handleShortcut(null);
    expect(useUiStore.getState().paletteOpen).toBe(false);
  });

  it('disables the context menu when asked', () => {
    vi.stubGlobal('isTauri', true);
    const cleanup = installDesktopEvents({ disableContextMenu: true });
    const ev = new MouseEvent('contextmenu', { cancelable: true });
    window.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    cleanup();
    const ev2 = new MouseEvent('contextmenu', { cancelable: true });
    window.dispatchEvent(ev2);
    expect(ev2.defaultPrevented).toBe(false);
  });
});
