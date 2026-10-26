import { create } from '@bufbuild/protobuf';
import { ActionInputSchema, OptionalStringListSchema, EntryType } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { LaunchAction } from '@latch/rpc/v1';
import { Button } from '@latch/ui/button';
import { Checkbox } from '@latch/ui/checkbox';
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger
} from '@latch/ui/dialog';
import { Input } from '@latch/ui/input';
import { Spinner } from '@latch/ui/spinner';
import { Textarea } from '@latch/ui/textarea';
import { FolderOpenIcon, PencilIcon, PlayIcon, PlusIcon, TrashIcon } from 'lucide-react';
import { useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { FormField } from '~/components/fields';
import { errorMessage } from '~/lib/api/errors';
import { qk, useActions, useEntries, useRpcMutation } from '~/lib/api/queries';
import { isTauri } from '~/lib/api/transport';
import { pickPath } from '~/lib/clipboard';
import { notify } from '~/lib/notify';

const emptyInput = () =>
  create(ActionInputSchema, {
    name: '',
    command: '',
    envEntryIds: [],
    envKeys: undefined,
    urls: []
  });

export function ActionsButton({ profileId }: { profileId: string }) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger
        render={
          <Button aria-label="Actions" size="sm" variant="outline" className="px-2 lg:px-2.5" />
        }
      >
        <PlayIcon /> <span className="hidden lg:inline">Actions</span>
      </DialogTrigger>
      <DialogPopup className="[--glass-opacity:100%] sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Actions</DialogTitle>
          <DialogDescription>
            Launch commands in an interactive terminal and open URLs in this profile’s browser.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel>{open ? <ActionsPanel profileId={profileId} /> : null}</DialogPanel>
      </DialogPopup>
    </Dialog>
  );
}

function ActionsPanel({ profileId }: { profileId: string }) {
  const actions = useActions(profileId);
  const [editing, setEditing] = useState<LaunchAction | 'new' | null>(null);
  const [deleting, setDeleting] = useState<LaunchAction | null>(null);
  const remove = useRpcMutation(LatchAgent.method.actionsDelete, {
    invalidate: [qk.actionList(profileId)],
    onSuccess: () => {
      setDeleting(null);
      notify.success('Action deleted');
    },
    onError: (error) => notify.error('Could not delete action', error)
  });
  const run = useRpcMutation(LatchAgent.method.actionsRun, {
    invalidate: [qk.browser, qk.profiles],
    onSuccess: () => notify.success('Action launched'),
    onError: (error) => notify.error('Could not launch action', error)
  });
  if (actions.isPending)
    return (
      <div className="flex items-center gap-2 text-sm" role="status">
        <Spinner /> Loading actions…
      </div>
    );
  if (actions.isError)
    return (
      <div className="space-y-3">
        <p role="alert" className="text-destructive text-sm">
          {errorMessage(actions.error)}
        </p>
        <Button variant="outline" onClick={() => void actions.refetch()}>
          Retry
        </Button>
      </div>
    );
  if (editing)
    return (
      <ActionEditor
        key={editing === 'new' ? 'new' : editing.id}
        profileId={profileId}
        action={editing === 'new' ? null : editing}
        onClose={() => setEditing(null)}
      />
    );
  return (
    <div className="space-y-6">
      <DirectoryForm
        key={actions.data.workingDirectory ?? ''}
        profileId={profileId}
        initial={actions.data.workingDirectory}
      />
      <section className="space-y-3" aria-label="Saved actions">
        <div className="flex items-center justify-between gap-2">
          <h3 className="font-medium text-sm">Saved actions</h3>
          <Button size="sm" onClick={() => setEditing('new')}>
            <PlusIcon /> New action
          </Button>
        </div>
        {actions.data.actions.length === 0 ? (
          <div className="py-6 text-center">
            <p className="font-medium text-sm">No actions yet</p>
            <p className="mt-1 text-muted-foreground text-sm">
              Save a command, browser URLs, or both to start your project.
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-border">
            {actions.data.actions.map((action) => (
              <li key={action.id} className="flex items-center gap-2 py-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium text-sm">{action.name}</p>
                  <p className="truncate text-muted-foreground text-xs">
                    {action.command || action.urls.join(', ')}
                  </p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  aria-label={`Play ${action.name}`}
                  disabled={run.isPending || remove.isPending}
                  onClick={() => run.mutate({ profileId, actionId: action.id })}
                >
                  {run.isPending ? <Spinner /> : <PlayIcon />} Play
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={`Edit ${action.name}`}
                  disabled={run.isPending || remove.isPending}
                  onClick={() => setEditing(action)}
                >
                  <PencilIcon />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={`Delete ${action.name}`}
                  disabled={run.isPending || remove.isPending}
                  onClick={() => setDeleting(action)}
                >
                  <TrashIcon />
                </Button>
              </li>
            ))}
          </ul>
        )}
        <p className="text-muted-foreground text-xs">
          Actions are encrypted and shared with the profile. URLs open after launch; Latch does not
          wait for a server to be ready.
        </p>
      </section>
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(value) => {
          if (!value && !remove.isPending) setDeleting(null);
        }}
        title={`Delete ${deleting?.name ?? 'action'}?`}
        description="This removes the saved action from this profile on every synced machine."
        confirmLabel="Delete action"
        pending={remove.isPending}
        onConfirm={() => {
          if (deleting) remove.mutate({ profileId, actionId: deleting.id });
        }}
      />
    </div>
  );
}

function DirectoryForm({ profileId, initial }: { profileId: string; initial: string | undefined }) {
  const [path, setPath] = useState(initial ?? '');
  const configure = useRpcMutation(LatchAgent.method.actionsConfigure, {
    invalidate: [qk.actionList(profileId)],
    onSuccess: () => notify.success('Project directory saved'),
    onError: (error) => notify.error('Could not save project directory', error)
  });
  return (
    <form
      className="space-y-2"
      onSubmit={(event) => {
        event.preventDefault();
        configure.mutate({ profileId, workingDirectory: path.trim() || undefined });
      }}
    >
      <FormField
        label="Project directory"
        description="Only on this machine. Required for commands; browser-only actions do not need it. Use an absolute path outside the vault."
      >
        {(attrs) => (
          <div className="flex gap-2">
            <Input
              {...attrs}
              value={path}
              disabled={configure.isPending}
              onChange={(event) => setPath(event.currentTarget.value)}
            />
            {isTauri() ? (
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="Browse project directory"
                disabled={configure.isPending}
                onClick={async () => {
                  try {
                    const picked = await pickPath({ directory: true, title: 'Project directory' });
                    if (picked) setPath(picked);
                  } catch (error) {
                    notify.error('Could not choose project directory', error);
                  }
                }}
              >
                <FolderOpenIcon />
              </Button>
            ) : null}
          </div>
        )}
      </FormField>
      <Button
        type="submit"
        size="sm"
        variant="outline"
        disabled={configure.isPending || path === (initial ?? '')}
      >
        {configure.isPending ? <Spinner /> : null}Save directory
      </Button>
    </form>
  );
}

function ActionEditor({
  profileId,
  action,
  onClose
}: {
  profileId: string;
  action: LaunchAction | null;
  onClose: () => void;
}) {
  const [input, setInput] = useState(() =>
    action
      ? create(ActionInputSchema, {
          name: action.name,
          command: action.command,
          envEntryIds: [...action.envEntryIds],
          envKeys: action.envKeys,
          urls: [...action.urls]
        })
      : emptyInput()
  );
  const [keys, setKeys] = useState(() => action?.envKeys?.items.join(', ') ?? '');
  const [urls, setUrls] = useState(() => action?.urls.join('\n') ?? '');
  const entries = useEntries({ profileId, type: EntryType.ENV });
  const selectedIds = new Set(input.envEntryIds);
  const availableIds = new Set(entries.data?.map((entry) => entry.id));
  const missingIds = input.envEntryIds.filter((id) => !availableIds.has(id));
  const save = useRpcMutation(LatchAgent.method.actionsSave, {
    invalidate: [qk.actionList(profileId)],
    onSuccess: () => {
      notify.success('Action saved');
      onClose();
    },
    onError: (error) => notify.error('Could not save action', error)
  });
  return (
    <form
      className="space-y-4"
      onSubmit={(event) => {
        event.preventDefault();
        save.mutate({
          profileId,
          id: action?.id,
          input: {
            ...input,
            envKeys: keys.trim()
              ? create(OptionalStringListSchema, { items: keys.split(/[\s,]+/).filter(Boolean) })
              : undefined,
            urls: urls
              .split('\n')
              .map((url) => url.trim())
              .filter(Boolean)
          }
        });
      }}
    >
      <h3 className="font-medium text-sm">{action ? 'Edit action' : 'New action'}</h3>
      <fieldset disabled={save.isPending} className="space-y-4">
        <FormField label="Name">
          {(attrs) => (
            <Input
              {...attrs}
              autoFocus
              required
              value={input.name}
              onChange={(event) => setInput({ ...input, name: event.currentTarget.value })}
            />
          )}
        </FormField>
        <FormField
          label="Command"
          description="Optional when URLs are provided. Runs in a fresh interactive terminal, which stays open after completion."
        >
          {(attrs) => (
            <Textarea
              {...attrs}
              className="font-mono"
              rows={2}
              value={input.command}
              onChange={(event) => setInput({ ...input, command: event.currentTarget.value })}
            />
          )}
        </FormField>
        <fieldset className="space-y-2">
          <legend className="mb-2 font-medium text-sm">Environment entries</legend>
          <p className="text-muted-foreground text-xs">
            Select which env entries to inject. Values stay out of this window.
          </p>
          {entries.isPending ? (
            <p role="status" className="text-sm">
              Loading env entries…
            </p>
          ) : entries.isError ? (
            <div>
              <p role="alert" className="text-destructive text-sm">
                Could not load env entries.
              </p>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void entries.refetch()}
              >
                Retry env entries
              </Button>
            </div>
          ) : entries.data?.length === 0 ? (
            <p className="text-muted-foreground text-sm">No env entries in this profile.</p>
          ) : (
            entries.data?.map((entry) => (
              <label key={entry.id} className="flex items-center gap-2 text-sm">
                <Checkbox
                  checked={selectedIds.has(entry.id)}
                  onCheckedChange={(checked) =>
                    setInput({
                      ...input,
                      envEntryIds: checked
                        ? [...input.envEntryIds, entry.id]
                        : input.envEntryIds.filter((id) => id !== entry.id)
                    })
                  }
                />
                {entry.name}
              </label>
            ))
          )}
          {entries.isSuccess && missingIds.length > 0 ? (
            <p role="alert" className="text-destructive text-xs">
              An environment source is unavailable. Remove it or restore the entry before running.
            </p>
          ) : null}
          {entries.isSuccess
            ? missingIds.map((id) => (
                <Button
                  key={id}
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    setInput({
                      ...input,
                      envEntryIds: input.envEntryIds.filter((source) => source !== id)
                    })
                  }
                >
                  Remove unavailable source {id}
                </Button>
              ))
            : null}
        </fieldset>
        <FormField
          label="Variable names"
          description="Optional: comma or space separated names. Leave blank to include every variable from the selected entries."
        >
          {(attrs) => (
            <Input
              {...attrs}
              value={keys}
              onChange={(event) => setKeys(event.currentTarget.value)}
            />
          )}
        </FormField>
        <FormField
          label="Browser URLs"
          description="Optional HTTP(S) URLs, one per line. Opens in this profile’s isolated browser."
        >
          {(attrs) => (
            <Textarea
              {...attrs}
              rows={3}
              value={urls}
              onChange={(event) => setUrls(event.currentTarget.value)}
            />
          )}
        </FormField>
      </fieldset>
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" disabled={save.isPending} onClick={onClose}>
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={save.isPending || !input.name.trim() || (!input.command.trim() && !urls.trim())}
        >
          {save.isPending ? <Spinner /> : null}Save action
        </Button>
      </div>
    </form>
  );
}
