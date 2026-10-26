import { isTauri } from '~/lib/api/transport';
import { useUiStore } from '~/stores/ui';

/** Emitted by the desktop shell when the global shortcut fires (or `latch-desktop --palette`). */
export const SHORTCUT_EVENT = 'latch://shortcut';

export interface ShortcutPayload {
  action: 'palette';
}

export function handleShortcut(payload: ShortcutPayload | null | undefined): void {
  if (payload?.action === 'palette') useUiStore.getState().setPaletteOpen(true);
}

function preventContextMenu(event: MouseEvent): void {
  event.preventDefault();
}

/**
 * Desktop-only hooks. No-op outside Tauri. Returns a cleanup function.
 * - `latch://shortcut` → open the quick-search palette.
 * - release builds: disable the webview's right-click context menu (SPEC §9.1).
 */
export function installDesktopEvents(options: { disableContextMenu?: boolean } = {}): () => void {
  if (!isTauri()) return () => {};
  const disableContextMenu = options.disableContextMenu ?? import.meta.env.PROD;
  if (disableContextMenu) window.addEventListener('contextmenu', preventContextMenu);

  let disposed = false;
  let unlisten: (() => void) | null = null;
  void import('@tauri-apps/api/event').then(({ listen }) =>
    listen<ShortcutPayload>(SHORTCUT_EVENT, (event) => handleShortcut(event.payload)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
  );

  return () => {
    disposed = true;
    if (disableContextMenu) window.removeEventListener('contextmenu', preventContextMenu);
    unlisten?.();
  };
}
