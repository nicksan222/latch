import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@latch/ui/empty';
import { Menu, MenuItem, MenuPopup, MenuSeparator, MenuTrigger } from '@latch/ui/menu';
import { Skeleton } from '@latch/ui/skeleton';
import { Spinner } from '@latch/ui/spinner';
import { Tabs, TabsList, TabsPanel, TabsTab } from '@latch/ui/tabs';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useParams, useRouter } from '@tanstack/react-router';
import {
  ArrowRightLeftIcon,
  EllipsisIcon,
  HistoryIcon,
  PencilIcon,
  RotateCcwIcon,
  SaveIcon,
  StarIcon,
  Trash2Icon
} from 'lucide-react';
import { useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { EntryTypeIcon, ProfileAvatar } from '~/components/icons';
import { ContentTitle, PageContent } from '~/components/page';
import { SimpleSelect } from '~/components/simple-select';
import { call } from '~/lib/api/client';
import { isRpcError } from '~/lib/api/errors';
import {
  qk,
  useEntry,
  useEntryHistory,
  useProfiles,
  useRpcMutation,
  useSettings
} from '~/lib/api/queries';
import { useAsyncAction } from '~/lib/async-ui';
import { ENTRY_TYPE_LABEL, entryToInput } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { formatDateTime, relativeTime } from '~/lib/time';
import { validateEntryInput, type FieldErrors } from '~/lib/validation';
import type { Entry, EntryInput, HistoryItem, ProfileSummary } from '../../_generated/contracts';
import { EntryForm } from './entry-form';
import { EntryView } from './entry-view';

export function EntryDetail() {
  const { entryId } = useParams({ from: '/p/$profileId/e/$entryId' });
  // Keyed: navigating to another entry unmounts the old one, dropping any
  // revealed value or edit buffer (AGENTS.md rule 7).
  return <EntryDetailInner entryId={entryId} key={entryId} />;
}

function HistoryTab({ entry }: { entry: Entry }) {
  const history = useEntryHistory(entry.id);
  const [target, setTarget] = useState<HistoryItem | null>(null);
  const restore = useRpcMutation('entries.restore', {
    invalidate: [qk.entries, qk.profiles],
    onSuccess: () => {
      notify.success('Version restored', 'A new commit records the restore.');
      setTarget(null);
    },
    onError: (e) => notify.error('Could not restore', e)
  });
  if (history.isPending)
    return (
      <div className="flex flex-col gap-2">
        {Array.from({ length: 3 }, (_, i) => (
          <Skeleton className="h-12" key={i} />
        ))}
      </div>
    );
  const items = history.data ?? [];
  return (
    <>
      <ol
        aria-label="Entry history"
        className="relative flex flex-col gap-0 border-s border-border/70 ps-4"
      >
        {items.map((item, i) => (
          <li
            className="relative flex items-start gap-3 py-2.5"
            data-testid="history-item"
            key={item.commit}
          >
            <span
              aria-hidden
              className="-start-[calc(--spacing(4)+4.5px)] absolute top-4 size-2 rounded-full border-2 border-background bg-muted-foreground/60"
            />
            <div className="flex min-w-0 basis-36 flex-1 flex-col gap-1">
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="font-medium text-sm">{item.message}</span>
                {i === 0 ? <Badge variant="info">current</Badge> : null}
                {item.deleted ? <Badge variant="error">deleted</Badge> : null}
              </div>
              <div className="flex flex-wrap items-center gap-1.5 text-muted-foreground text-xs">
                <span title={formatDateTime(item.at)}>{relativeTime(item.at)}</span>
                <span>·</span>
                <span>{item.author}</span>
                <span>·</span>
                <code className="font-mono">{item.commit.slice(0, 7)}</code>
              </div>
              {item.changedFields.length > 0 ? (
                <div className="flex flex-wrap gap-1">
                  {item.changedFields.map((f) => (
                    <Badge key={f} variant="outline">
                      {f}
                    </Badge>
                  ))}
                </div>
              ) : null}
            </div>
            {i > 0 && !item.deleted ? (
              <Button size="xs" variant="outline" onClick={() => setTarget(item)}>
                <RotateCcwIcon /> Restore
              </Button>
            ) : null}
          </li>
        ))}
      </ol>
      <ConfirmDialog
        confirmLabel="Restore this version"
        description={
          target
            ? `Replace the current version of “${entry.name}” with the one from ${formatDateTime(target.at)} (${target.commit.slice(0, 7)}). The current version stays in history.`
            : undefined
        }
        destructive={false}
        open={target !== null}
        pending={restore.isPending}
        title="Restore version?"
        onConfirm={() => target && restore.mutate({ id: entry.id, commit: target.commit })}
        onOpenChange={(open) => !open && setTarget(null)}
      />
    </>
  );
}

function DangerZone({ entry }: { entry: Entry }) {
  const router = useRouter();
  const stillViewingEntry = () =>
    router.state.location.pathname === `/p/${entry.profileId}/e/${entry.id}`;
  const profiles = useProfiles();
  const navigate = useNavigate();
  const [moveTo, setMoveTo] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const move = useRpcMutation('entries.move', {
    invalidate: [qk.entries, qk.profiles],
    onSuccess: (moved) => {
      notify.success(
        `Moved to ${profiles.data?.find((p) => p.id === moved.profileId)?.name ?? 'profile'}`
      );
      if (!stillViewingEntry()) return;
      void navigate({
        to: '/p/$profileId/e/$entryId',
        params: { profileId: moved.profileId, entryId: moved.id }
      });
    },
    onError: (e) => notify.error('Could not move entry', e)
  });
  const remove = useRpcMutation('entries.delete', {
    invalidate: [qk.entries, qk.profiles],
    onSuccess: () => {
      notify.success(`Deleted ${entry.name}`, 'Recoverable from Trash for 30 days.');
      if (!stillViewingEntry()) return;
      setConfirmDelete(false);
      void navigate({ to: '/p/$profileId', params: { profileId: entry.profileId } });
    },
    onError: (e) => notify.error('Could not delete entry', e)
  });
  const others = (profiles.data ?? []).filter((p) => p.id !== entry.profileId);
  return (
    <section
      aria-labelledby="danger-title"
      className="flex flex-col gap-3 rounded-xl border border-destructive/24 p-4"
    >
      <h3 className="font-medium text-destructive-foreground text-sm" id="danger-title">
        Danger zone
      </h3>
      <div className="flex flex-wrap items-center gap-2">
        <SimpleSelect
          aria-label="Move to profile"
          className="w-48"
          disabled={others.length === 0}
          options={others.map((p) => ({ value: p.id, label: p.name }))}
          placeholder="Move to profile…"
          size="sm"
          value={moveTo}
          onValueChange={setMoveTo}
        />
        <Button
          disabled={!moveTo || move.isPending}
          size="sm"
          variant="outline"
          onClick={() => moveTo && move.mutate({ id: entry.id, profileId: moveTo })}
        >
          <ArrowRightLeftIcon /> Move
        </Button>
        <Button
          className="ms-auto"
          size="sm"
          variant="destructive-outline"
          onClick={() => setConfirmDelete(true)}
        >
          <Trash2Icon /> Delete entry
        </Button>
      </div>
      <ConfirmDialog
        confirmLabel="Delete entry"
        description="The entry is removed from the vault. It stays in git history and can be restored from Trash for 30 days."
        open={confirmDelete}
        pending={remove.isPending}
        title={`Delete “${entry.name}”?`}
        onConfirm={() => remove.mutate({ id: entry.id })}
        onOpenChange={setConfirmDelete}
      />
    </section>
  );
}

function EntryDetailInner({ entryId }: { entryId: string }) {
  const entry = useEntry(entryId);
  const settings = useSettings();
  const profiles = useProfiles();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'details' | 'history'>('details');
  const [draft, setDraft] = useState<EntryInput | null>(null);
  const [errors, setErrors] = useState<FieldErrors>({});
  const editor = useAsyncAction(draft);
  const validation = useAsyncAction(draft);
  const update = useRpcMutation('entries.update', {
    invalidate: [qk.entries, qk.profiles],
    onSuccess: () => {
      notify.success('Saved');
      setDraft(null);
      setErrors({});
    },
    onError: (e) => notify.error('Could not save', e)
  });
  const favorite = useRpcMutation('entries.update', { invalidate: [qk.entries, qk.totpPinned] });
  const hideAfterSecs = settings.data?.revealHideSecs ?? 20;

  if (entry.isPending)
    return (
      <div className="flex flex-col gap-4 p-6">
        <Skeleton className="h-8 w-1/3" />
        <Skeleton className="h-24" />
        <Skeleton className="h-24" />
      </div>
    );
  if (entry.isError || !entry.data)
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>
            {isRpcError(entry.error, 'not_found') ? 'Entry not found' : 'Could not load entry'}
          </EmptyTitle>
          <EmptyDescription>It may have been deleted or moved.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );

  const e = entry.data;
  const profile = profiles.data?.find((p) => p.id === e.profileId);

  /** Explicit action: fetch the revealed entry for editing. */
  const startEdit = async () => {
    await editor.run(
      () => call('entries.get', { id: e.id, reveal: true }),
      (full) => {
        setDraft(entryToInput(full));
        setTab('details');
      },
      (error) => notify.error('Could not open editor', error)
    );
  };

  const save = () => {
    if (!draft) return;
    void validation.run(
      () => validateEntryInput(draft),
      (errs) => {
        setErrors(errs);
        if (Object.keys(errs).length > 0) {
          notify.error('Fix the highlighted fields');
          return;
        }
        update.mutate({ id: e.id, input: draft });
      },
      (error) => notify.error('Could not validate entry', error)
    );
  };

  return (
    <PageContent data-testid="entry-detail">
      <div className="flex flex-wrap items-start gap-3">
        <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border bg-card text-muted-foreground">
          <EntryTypeIcon className="size-5" type={e.payload.type} />
        </span>
        <EntryHeading e={e} profile={profile} />
        <EntryActions
          e={e}
          editing={draft !== null}
          pending={update.isPending || validation.pending}
          loadingEdit={editor.pending}
          save={save}
          startEdit={startEdit}
          onCancel={() => {
            setDraft(null);
            setErrors({});
          }}
          onFavorite={() =>
            favorite.mutate({ id: e.id, input: { ...entryToInput(e), favorite: !e.favorite } })
          }
          onHistory={() => setTab('history')}
          onReload={() => void queryClient.invalidateQueries({ queryKey: qk.entry(e.id) })}
        />
      </div>

      <Tabs value={tab} onValueChange={(v) => setTab(v as 'details' | 'history')}>
        <TabsList variant="underline">
          <TabsTab value="details">Details</TabsTab>
          <TabsTab disabled={draft !== null} value="history">
            History
          </TabsTab>
        </TabsList>
        <TabsPanel className="pt-4" value="details">
          {draft ? (
            <EntryForm errors={errors} value={draft} onChange={setDraft} />
          ) : (
            <div className="flex flex-col gap-6">
              <EntryView
                entry={e}
                hideAfterSecs={hideAfterSecs}
                profileName={profile?.name ?? ''}
              />
              {e.notes ? (
                <div className="flex flex-col gap-1.5">
                  <span className="font-medium text-muted-foreground text-xs">Notes</span>
                  <p className="whitespace-pre-wrap text-sm">{e.notes}</p>
                </div>
              ) : null}
              <p className="text-muted-foreground text-xs">
                Created {formatDateTime(e.createdAt)} · Updated {formatDateTime(e.updatedAt)}
              </p>
              <DangerZone entry={e} />
            </div>
          )}
        </TabsPanel>
        <TabsPanel className="pt-4" value="history">
          <HistoryTab entry={e} />
        </TabsPanel>
      </Tabs>
    </PageContent>
  );
}

function EntryHeading({ e, profile }: { e: Entry; profile: ProfileSummary | undefined }) {
  return (
    <div className="flex min-w-0 basis-36 flex-1 flex-col gap-1">
      <ContentTitle>{e.name}</ContentTitle>
      <div className="flex flex-wrap items-center gap-1.5 text-muted-foreground text-xs">
        <Badge variant="secondary">{ENTRY_TYPE_LABEL[e.payload.type]}</Badge>
        {profile ? (
          <span className="inline-flex items-center gap-1">
            <ProfileAvatar color={profile.color} icon={profile.icon} size="xs" /> {profile.name}
          </span>
        ) : null}
        <span>· updated {relativeTime(e.updatedAt)}</span>
        {e.tags.map((t) => (
          <Badge key={t} variant="outline">
            {t}
          </Badge>
        ))}
      </div>
    </div>
  );
}

function EntryActions({
  e,
  editing,
  pending,
  loadingEdit,
  save,
  startEdit,
  onCancel,
  onFavorite,
  onHistory,
  onReload
}: {
  e: Entry;
  editing: boolean;
  pending: boolean;
  loadingEdit: boolean;
  save: () => void;
  startEdit: () => Promise<void>;
  onCancel: () => void;
  onFavorite: () => void;
  onHistory: () => void;
  onReload: () => void;
}) {
  return (
    <>
      {editing ? (
        <div className="flex shrink-0 items-center gap-2">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button disabled={pending} onClick={save}>
            {pending ? <Spinner /> : <SaveIcon />} Save
          </Button>
        </div>
      ) : (
        <div className="flex shrink-0 items-center gap-1">
          <Button
            aria-label={e.favorite ? 'Remove from favorites' : 'Add to favorites'}
            aria-pressed={e.favorite}
            size="icon-sm"
            variant="ghost"
            onClick={onFavorite}
          >
            <StarIcon className={e.favorite ? 'fill-warning text-warning' : undefined} />
          </Button>
          <Button disabled={loadingEdit} variant="outline" onClick={() => void startEdit()}>
            {loadingEdit ? <Spinner /> : <PencilIcon />} Edit
          </Button>
          <Menu>
            <MenuTrigger
              render={<Button aria-label="More actions" size="icon-sm" variant="ghost" />}
            >
              <EllipsisIcon />
            </MenuTrigger>
            <MenuPopup align="end">
              <MenuItem onClick={onHistory}>
                <HistoryIcon /> View history
              </MenuItem>
              <MenuSeparator />
              <MenuItem onClick={onReload}>
                <RotateCcwIcon /> Reload
              </MenuItem>
            </MenuPopup>
          </Menu>
        </div>
      )}
    </>
  );
}
