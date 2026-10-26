import { LatchAgent } from '@latch/rpc/v1';
import { Button } from '@latch/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@latch/ui/empty';
import { Skeleton } from '@latch/ui/skeleton';
import { RotateCcwIcon, Trash2Icon } from 'lucide-react';
import { EntryTypeIcon, ProfileDot } from '~/components/icons';
import { PageContainer, PageTitle, SettingsSection } from '~/components/page';
import { AppHeader } from '~/features/shell/app-header';
import { qk, useProfiles, useRpcMutation, useTrash } from '~/lib/api/queries';
import { ENTRY_TYPE_LABEL } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { formatDateTime, relativeTime } from '~/lib/time';

export function TrashPage() {
  const trash = useTrash();
  const profiles = useProfiles();
  const restore = useRpcMutation(LatchAgent.method.entriesRestore, {
    invalidate: [qk.entries, qk.profiles],
    onSuccess: (e) => notify.success(`Restored ${e.name}`),
    onError: (e) => notify.error('Could not restore', e)
  });
  const byId = new Map((profiles.data ?? []).map((p) => [p.id, p]));
  return (
    <>
      <AppHeader title={<PageTitle>Trash</PageTitle>} />
      <PageContainer>
        <p className="text-muted-foreground text-sm">
          Deleted entries stay recoverable from git history for 30 days.
        </p>
        {trash.isPending ? (
          <Skeleton className="h-32 rounded-xl" />
        ) : (trash.data ?? []).length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Trash2Icon />
              </EmptyMedia>
              <EmptyTitle>Trash is empty</EmptyTitle>
              <EmptyDescription>Nothing has been deleted recently.</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <SettingsSection title="Recently deleted">
            {trash.data!.map((t) => {
              const profile = byId.get(t.profileId);
              return (
                <div
                  className="flex items-center gap-3 px-3 py-2.5 sm:px-4"
                  data-testid="trash-row"
                  key={t.id}
                >
                  <EntryTypeIcon className="text-muted-foreground" type={t.type} />
                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate font-medium text-sm">{t.name}</span>
                    <span className="flex flex-wrap items-center gap-1.5 break-words text-muted-foreground text-xs">
                      {profile ? <ProfileDot color={profile.color} /> : null}
                      {profile?.name ?? 'deleted profile'} · {ENTRY_TYPE_LABEL[t.type]} ·{' '}
                      <span title={formatDateTime(t.deletedAt)}>
                        deleted {relativeTime(t.deletedAt)}
                      </span>
                    </span>
                  </div>
                  <Button
                    disabled={!profile || restore.isPending}
                    size="sm"
                    variant="outline"
                    onClick={() => restore.mutate({ id: t.id, commit: t.commit })}
                  >
                    <RotateCcwIcon /> Restore
                  </Button>
                </div>
              );
            })}
          </SettingsSection>
        )}
      </PageContainer>
    </>
  );
}
