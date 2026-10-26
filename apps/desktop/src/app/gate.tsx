import { Button } from '@latch/ui/button';
import { Spinner } from '@latch/ui/spinner';
import { useQueryClient } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { RefreshCwIcon } from 'lucide-react';
import { useCallback, useEffect, type ReactNode } from 'react';
import { LockScreen } from '~/features/lock/lock-screen';
import { Onboarding } from '~/features/onboarding/onboarding';
import { AppShell } from '~/features/shell/app-shell';
import { errorMessage } from '~/lib/api/errors';
import {
  clearMutations,
  scrubOnLock,
  useAgentEvents,
  useSettings,
  useVaultStatus
} from '~/lib/api/queries';
import { notify } from '~/lib/notify';
import { applyTheme, useThemeStore } from '~/stores/theme';
import { useUiStore } from '~/stores/ui';
import type { AgentEvent } from '../_generated/contracts';

function useThemeSync(enabled: boolean) {
  const settings = useSettings(enabled);
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const remoteTheme = settings.data?.theme;
  // Adopt the stored setting whenever it (re)loads; local changes write through.
  useEffect(() => {
    if (remoteTheme) setTheme(remoteTheme);
  }, [remoteTheme, setTheme]);
  useEffect(() => {
    applyTheme(theme);
    if (theme !== 'system') return;
    const mq = window.matchMedia?.('(prefers-color-scheme: dark)');
    const onChange = () => applyTheme('system');
    mq?.addEventListener?.('change', onChange);
    return () => mq?.removeEventListener?.('change', onChange);
  }, [theme]);
}

/** Decides between onboarding, the lock screen and the unlocked app. */
export function Gate({ children }: { children: ReactNode }) {
  const status = useVaultStatus();
  const addingVault = useUiStore((s) => s.addingVault);
  const onEvent = useCallback((event: AgentEvent) => {
    if (event.kind === 'lock_warning')
      notify.warning('Vault will lock soon', event.message ?? undefined);
    if (event.kind === 'locked' && event.message) notify.info('Vault locked', event.message);
  }, []);
  useAgentEvents(onEvent);
  useThemeSync(status.data?.initialized === true);

  // Plaintext hygiene (AGENTS.md rule 7): a locked status (even without a
  // `locked` event) drops cached metadata and every mutation's variables;
  // navigation drops mutation variables too.
  const queryClient = useQueryClient();
  const locked = status.data?.locked === true;
  useEffect(() => {
    if (locked) scrubOnLock(queryClient);
  }, [locked, queryClient]);
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  useEffect(() => {
    clearMutations(queryClient);
  }, [pathname, queryClient]);

  if (status.isPending) {
    return (
      <div className="flex h-full items-center justify-center bg-background" aria-busy="true">
        <Spinner size="lg" tone="muted" />
      </div>
    );
  }
  if (status.isError) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 bg-background p-6 text-center">
        <p className="font-medium">Cannot reach the Latch agent</p>
        <p className="max-w-sm text-muted-foreground text-sm">{errorMessage(status.error)}</p>
        <Button variant="outline" onClick={() => void status.refetch()}>
          <RefreshCwIcon /> Retry
        </Button>
      </div>
    );
  }
  if (!status.data.initialized || addingVault) return <Onboarding />;
  if (status.data.locked) return <LockScreen status={status.data} />;
  return <AppShell>{children}</AppShell>;
}
