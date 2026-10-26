import { LatchAgent } from '@latch/rpc/v1';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { useNow } from '@latch/ui/hooks/use-now';
import { Kbd } from '@latch/ui/kbd';
import { SidebarTrigger } from '@latch/ui/sidebar';
import { Tooltip, TooltipPopup, TooltipTrigger } from '@latch/ui/tooltip';
import { isMacPlatform } from '@latch/ui/utils';
import { useQueryClient } from '@tanstack/react-query';
import { LockIcon, SearchIcon, TimerIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { PageHeader } from '~/components/page';
import { call } from '~/lib/api/client';
import { qk, useVaultStatus } from '~/lib/api/queries';
import { notify } from '~/lib/notify';
import { formatDuration } from '~/lib/time';
import { useUiStore } from '~/stores/ui';

export const MOD_KEY =
  typeof navigator !== 'undefined' && isMacPlatform(navigator.platform) ? '⌘' : 'Ctrl';

/** Seconds until auto-lock, extrapolated locally from the last `vault.status`. */
export function useAutoLockCountdown(): number | null {
  const status = useVaultStatus(15_000);
  const now = useNow(1000);
  const secs = status.data?.locksInSecs;
  if (secs === null || secs === undefined) return null;
  return Math.max(0, Number(secs) - Math.floor((now - status.dataUpdatedAt) / 1000));
}

export function LockButton() {
  const queryClient = useQueryClient();
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            aria-label="Lock vault"
            size="icon-sm"
            variant="ghost"
            onClick={async () => {
              try {
                await call(LatchAgent.method.vaultLock);
              } catch (error) {
                notify.error('Could not lock', error);
              }
              await queryClient.invalidateQueries({ queryKey: qk.status });
            }}
          />
        }
      >
        <LockIcon />
      </TooltipTrigger>
      <TooltipPopup>Lock vault</TooltipPopup>
    </Tooltip>
  );
}

export function AppHeader({ title, actions }: { title: ReactNode; actions?: ReactNode }) {
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);
  const locksIn = useAutoLockCountdown();
  return (
    <PageHeader>
      <SidebarTrigger className="shrink-0 md:hidden" />
      <div className="flex min-w-0 flex-1 items-center gap-2">{title}</div>
      <div className="flex shrink-0 items-center gap-1.5">
        {actions}
        {locksIn !== null && locksIn <= 60 ? (
          <Badge variant="warning" size="lg" aria-live="polite" data-testid="autolock-countdown">
            <TimerIcon /> Locks in {formatDuration(locksIn)}
          </Badge>
        ) : null}
        <Button
          aria-label="Search entries"
          className="hidden gap-2 text-muted-foreground sm:inline-flex"
          size="sm"
          variant="outline"
          onClick={() => setPaletteOpen(true)}
        >
          <SearchIcon />
          <span className="pe-6">Search…</span>
          <Kbd>{MOD_KEY} K</Kbd>
        </Button>
        <Button
          aria-label="Search entries"
          className="sm:hidden"
          size="icon-sm"
          variant="ghost"
          onClick={() => setPaletteOpen(true)}
        >
          <SearchIcon />
        </Button>
        <LockButton />
      </div>
    </PageHeader>
  );
}
