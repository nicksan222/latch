import { LatchAgent } from '@latch/rpc/v1';
import { SyncMode, type ConflictSummary, type SyncStatus } from '@latch/rpc/v1';
import { Alert, AlertDescription, AlertTitle } from '@latch/ui/alert';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@latch/ui/empty';
import { Group } from '@latch/ui/group';
import { Input } from '@latch/ui/input';
import { Skeleton } from '@latch/ui/skeleton';
import { Spinner } from '@latch/ui/spinner';
import {
  ArrowDownIcon,
  ArrowDownUpIcon,
  ArrowUpIcon,
  CheckCircle2Icon,
  GitBranchIcon,
  GitMergeIcon,
  TriangleAlertIcon
} from 'lucide-react';
import { useState } from 'react';
import { PageContainer, PageTitle, SettingsRow, SettingsSection } from '~/components/page';
import { AppHeader } from '~/features/shell/app-header';
import { qk, useRpcMutation, useSyncStatus } from '~/lib/api/queries';
import { notify } from '~/lib/notify';
import { formatDateTime, relativeTime } from '~/lib/time';
import { ConflictResolver } from './conflict-resolver';

function Stat({
  label,
  value,
  icon
}: {
  label: string;
  value: React.ReactNode;
  icon: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1 rounded-xl border border-border/60 bg-card/40 p-3">
      <span className="flex items-center gap-1.5 text-muted-foreground text-xs">
        {icon}
        {label}
      </span>
      <span className="font-semibold text-lg tabular-nums">{value}</span>
    </div>
  );
}

function RemoteForm({ current }: { current: string | null }) {
  const [url, setUrl] = useState(current ?? '');
  const setRemote = useRpcMutation(LatchAgent.method.syncSetRemote, {
    invalidate: [qk.sync, qk.status],
    onSuccess: (s) => notify.success(s.remoteUrl ? 'Remote saved' : 'Remote removed'),
    onError: (e) => notify.error('Could not set remote', e)
  });
  return (
    <form
      className="flex w-full gap-2 sm:w-auto"
      onSubmit={(e) => {
        e.preventDefault();
        setRemote.mutate({ url: url.trim() });
      }}
    >
      <Input
        aria-label="Remote URL"
        className="font-mono sm:w-80"
        placeholder="git@github.com:you/vault.git"
        size="sm"
        value={url}
        onChange={(e) => setUrl(e.currentTarget.value)}
      />
      <Button
        disabled={setRemote.isPending || url.trim() === (current ?? '')}
        size="sm"
        type="submit"
        variant="outline"
      >
        {setRemote.isPending ? <Spinner /> : null}
        {url.trim() ? 'Save' : 'Remove'}
      </Button>
    </form>
  );
}

export function SyncPage() {
  const sync = useSyncStatus();
  const [resolving, setResolving] = useState<ConflictSummary | null>(null);
  const [runningMode, setRunningMode] = useState<SyncMode | null>(null);
  const run = useRpcMutation(LatchAgent.method.syncRun, {
    invalidate: [qk.sync, qk.status, qk.entries, qk.profiles],
    onSuccess: (r) => {
      setRunningMode(null);
      if (r.conflicts.length > 0)
        notify.warning(
          `${r.conflicts.length} conflict${r.conflicts.length === 1 ? '' : 's'} to resolve`,
          'Pulled changes touch entries you also changed.'
        );
      else
        notify.success(
          'Sync complete',
          `Pulled ${r.pulled}, pushed ${r.pushed} commit${r.pushed === 1 ? '' : 's'}.`
        );
    },
    onError: (e) => {
      setRunningMode(null);
      notify.error('Sync failed', e);
    }
  });
  const go = (mode: SyncMode) => {
    setRunningMode(mode);
    run.mutate({ mode });
  };
  const s = sync.data;

  return (
    <>
      <AppHeader
        title={<PageTitle>Sync</PageTitle>}
        actions={
          s?.hasRemote ? (
            <SyncActions pending={run.isPending} runningMode={runningMode} go={go} />
          ) : null
        }
      />
      <PageContainer>
        {sync.isPending || !s ? (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {Array.from({ length: 4 }, (_, i) => (
              <Skeleton className="h-16 rounded-xl" key={i} />
            ))}
          </div>
        ) : (
          <>
            <SyncStats s={s} />
            {s.dirty ? (
              <Alert variant="warning">
                <TriangleAlertIcon />
                <AlertTitle>Uncommitted changes</AlertTitle>
                <AlertDescription>
                  The vault repository has changes Latch did not commit. Run Doctor from Settings.
                </AlertDescription>
              </Alert>
            ) : null}

            {s.conflicts.length > 0 ? (
              <SettingsSection icon={<GitMergeIcon className="size-4" />} title="Conflicts">
                {s.conflicts.map((c) => (
                  <SettingsRow
                    control={
                      <Button size="sm" onClick={() => setResolving(c)}>
                        Resolve…
                      </Button>
                    }
                    description="Changed both here and on the remote."
                    key={c.entryId}
                    title={c.name}
                  />
                ))}
              </SettingsSection>
            ) : null}

            <SettingsSection title="Remote">
              <SettingsRow
                control={<RemoteForm current={s.remoteUrl ?? null} key={s.remoteUrl ?? ''} />}
                description={
                  s.hasRemote
                    ? 'Pushing is explicit unless auto-push is enabled in Settings.'
                    : 'No remote: the vault is only on this machine.'
                }
                title="Remote URL"
              />
            </SettingsSection>
            {!s.hasRemote ? (
              <Alert variant="info">
                <AlertDescription>
                  Add a remote (e.g. a private or even public GitHub repo) to back up and share the
                  vault.
                </AlertDescription>
              </Alert>
            ) : null}

            <RecentCommits s={s} />
          </>
        )}
      </PageContainer>
      {resolving ? (
        <ConflictResolver conflict={resolving} onClose={() => setResolving(null)} />
      ) : null}
    </>
  );
}

function SyncStats({ s }: { s: SyncStatus }) {
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="sync-stats">
      <Stat icon={<ArrowUpIcon className="size-3.5" />} label="Ahead" value={s.ahead} />
      <Stat icon={<ArrowDownIcon className="size-3.5" />} label="Behind" value={s.behind} />
      <Stat
        icon={<GitBranchIcon className="size-3.5" />}
        label="Branch"
        value={<span className="break-all text-base">{s.branch}</span>}
      />
      <Stat
        icon={<CheckCircle2Icon className="size-3.5" />}
        label="Last sync"
        value={
          <span
            className="text-base"
            title={s.lastSyncAt ? formatDateTime(s.lastSyncAt) : undefined}
          >
            {relativeTime(s.lastSyncAt)}
          </span>
        }
      />
    </div>
  );
}

function RecentCommits({ s }: { s: SyncStatus }) {
  return (
    <SettingsSection title="Recent commits">
      {s.log.length === 0 ? (
        <Empty size="compact">
          <EmptyHeader>
            <EmptyTitle>No commits</EmptyTitle>
            <EmptyDescription>Changes are committed automatically.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ol aria-label="Recent commits" className="divide-y divide-border/50">
          {s.log.slice(0, 20).map((c, i) => (
            <li className="flex items-center gap-3 px-3 py-2 sm:px-4" key={c.commit}>
              <code className="font-mono text-muted-foreground text-xs">
                {c.commit.slice(0, 7)}
              </code>
              <span className="min-w-0 flex-1 truncate text-sm">{c.message}</span>
              {i < s.ahead ? <Badge variant="info">unpushed</Badge> : null}
              <span className="hidden text-muted-foreground text-xs sm:inline">
                {c.author.replace(/ <.*>/, '')}
              </span>
              <span className="text-muted-foreground text-xs" title={formatDateTime(c.at)}>
                {relativeTime(c.at)}
              </span>
            </li>
          ))}
        </ol>
      )}
    </SettingsSection>
  );
}

function SyncActions({
  pending,
  runningMode,
  go
}: {
  pending: boolean;
  runningMode: SyncMode | null;
  go: (mode: SyncMode) => void;
}) {
  return (
    <Group>
      <Button disabled={pending} size="sm" onClick={() => go(SyncMode.BOTH)}>
        {runningMode === SyncMode.BOTH ? <Spinner /> : <ArrowDownUpIcon />} Sync now
      </Button>
      <Button
        aria-label="Pull only"
        disabled={pending}
        size="sm"
        variant="outline"
        onClick={() => go(SyncMode.PULL_ONLY)}
      >
        {runningMode === SyncMode.PULL_ONLY ? <Spinner /> : <ArrowDownIcon />} Pull
      </Button>
      <Button
        aria-label="Push only"
        disabled={pending}
        size="sm"
        variant="outline"
        onClick={() => go(SyncMode.PUSH_ONLY)}
      >
        {runningMode === SyncMode.PUSH_ONLY ? <Spinner /> : <ArrowUpIcon />} Push
      </Button>
    </Group>
  );
}
