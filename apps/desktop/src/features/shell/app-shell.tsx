import { SidebarInset, SidebarProvider } from '@latch/ui/sidebar';
import { useEffect, type ReactNode } from 'react';
import { CommandPalette } from '~/features/palette/command-palette';
import { useUiStore } from '~/stores/ui';
import { AppSidebar } from './app-sidebar';
import { VaultNotices } from './vault-notices';

/** Global in-app shortcuts: ⌘K / Ctrl+K opens the quick-search palette. */
function useGlobalShortcuts() {
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setPaletteOpen(!useUiStore.getState().paletteOpen);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [setPaletteOpen]);
}

export function AppShell({ children }: { children: ReactNode }) {
  useGlobalShortcuts();
  return (
    <SidebarProvider className="h-full min-h-0">
      <AppSidebar />
      <SidebarInset className="h-svh min-h-0 overflow-hidden">
        <VaultNotices />
        {children}
      </SidebarInset>
      <CommandPalette />
    </SidebarProvider>
  );
}
