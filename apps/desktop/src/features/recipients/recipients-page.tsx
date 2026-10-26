import { Alert, AlertDescription, AlertTitle } from '@latch/ui/alert';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle
} from '@latch/ui/dialog';
import { Input } from '@latch/ui/input';
import { Skeleton } from '@latch/ui/skeleton';
import { Spinner } from '@latch/ui/spinner';
import { Textarea } from '@latch/ui/textarea';
import { KeyRoundIcon, PlusIcon, RefreshCcwIcon, Trash2Icon } from 'lucide-react';
import { useMemo, useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { FormField } from '~/components/fields';
import { PageContainer, PageTitle, SettingsSection } from '~/components/page';
import { AppHeader } from '~/features/shell/app-header';
import { qk, useRecipients, useRpcMutation } from '~/lib/api/queries';
import { useAsyncAction, useAsyncValue } from '~/lib/async-ui';
import { notify } from '~/lib/notify';
import { relativeTime } from '~/lib/time';
import { recipientKeyKind, validateRecipient, type FieldErrors } from '~/lib/validation';
import type { RecipientInfo, RecipientKind } from '../../_generated/contracts';

const KIND_LABEL: Record<RecipientKind, string> = {
  passphrase: 'Passphrase',
  age: 'age',
  ssh: 'SSH',
  plugin: 'Plugin',
  recovery: 'Recovery kit'
};

function AddRecipientDialog({
  open,
  onOpenChange,
  existing
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  existing: RecipientInfo[];
}) {
  const [label, setLabel] = useState('');
  const [key, setKey] = useState('');
  const [errors, setErrors] = useState<FieldErrors>({});
  const add = useRpcMutation('recipients.add', {
    invalidate: [qk.recipients, qk.status],
    onSuccess: () => {
      notify.success(`Added ${label}`, 'The vault key was re-encrypted. Push to share access.');
      setLabel('');
      setKey('');
      onOpenChange(false);
    },
    onError: (e) => notify.error('Could not add recipient', e)
  });
  const kind = useAsyncValue(key, recipientKeyKind)?.value;
  const validationScope = useMemo(
    () => ({ label, key, open, existing }),
    [label, key, open, existing]
  );
  const validation = useAsyncAction(validationScope);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogPopup>
        <form
          className="contents"
          onSubmit={(e) => {
            e.preventDefault();
            void validation.run(
              () => validateRecipient(label, key, existing),
              (errs) => {
                setErrors(errs);
                if (Object.keys(errs).length === 0) add.mutate({ label, publicKey: key });
              },
              (error) => notify.error('Could not validate recipient', error)
            );
          }}
        >
          <DialogHeader>
            <DialogTitle>Add recipient</DialogTitle>
            <DialogDescription>
              A teammate or device that can unlock this vault with their own key.
            </DialogDescription>
          </DialogHeader>
          <DialogPanel className="flex flex-col gap-4">
            <FormField error={errors.label} label="Label">
              {(a) => (
                <Input
                  {...a}
                  autoFocus
                  placeholder="sam@laptop"
                  value={label}
                  onChange={(e) => setLabel(e.currentTarget.value)}
                />
              )}
            </FormField>
            <FormField
              description={
                kind
                  ? `Detected: ${KIND_LABEL[kind]} public key`
                  : 'Paste an age1… recipient or an SSH public key.'
              }
              error={errors.key}
              label="Public key"
            >
              {(a) => (
                <Textarea
                  {...a}
                  className="font-mono text-xs"
                  placeholder="age1…"
                  rows={3}
                  spellCheck={false}
                  value={key}
                  onChange={(e) => setKey(e.currentTarget.value)}
                />
              )}
            </FormField>
          </DialogPanel>
          <DialogFooter>
            <Button variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={add.isPending || validation.pending} type="submit">
              {add.isPending ? <Spinner /> : <PlusIcon />} Add recipient
            </Button>
          </DialogFooter>
        </form>
      </DialogPopup>
    </Dialog>
  );
}

export function RecipientsPage() {
  const recipients = useRecipients();
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<RecipientInfo | null>(null);
  const [rotating, setRotating] = useState(false);
  const remove = useRpcMutation('recipients.remove', {
    invalidate: [qk.recipients, qk.status],
    onSuccess: () => {
      notify.success(
        `Removed ${removing?.label ?? 'recipient'}`,
        'Consider rotating the vault key.'
      );
      setRemoving(null);
    },
    onError: (e) => notify.error('Could not remove recipient', e)
  });
  const rotate = useRpcMutation('recipients.rotate', {
    invalidate: [qk.recipients, qk.status, qk.sync],
    onSuccess: () => {
      notify.success('Vault key rotated', 'Every entry was re-encrypted in one commit.');
      setRotating(false);
    },
    onError: (e) => notify.error('Rotation failed', e)
  });
  const list = recipients.data ?? [];

  return (
    <>
      <AppHeader
        title={<PageTitle>Recipients</PageTitle>}
        actions={
          <>
            <Button size="sm" variant="outline" onClick={() => setRotating(true)}>
              <RefreshCcwIcon /> Rotate vault key
            </Button>
            <Button size="sm" onClick={() => setAdding(true)}>
              <PlusIcon /> Add recipient
            </Button>
          </>
        }
      />
      <PageContainer>
        <p className="text-muted-foreground text-sm">
          Recipients can decrypt the vault key. Adding or removing one re-encrypts only{' '}
          <code className="font-mono text-xs">vaultkey.age</code>.
        </p>
        <SettingsSection title={`${list.length} recipient${list.length === 1 ? '' : 's'}`}>
          {recipients.isPending ? (
            <div className="flex flex-col gap-2 p-4">
              <Skeleton className="h-10" />
              <Skeleton className="h-10" />
            </div>
          ) : (
            list.map((r) => (
              <div
                className="flex items-center gap-3 px-3 py-3 sm:px-4"
                data-testid="recipient-row"
                key={r.label}
              >
                <KeyRoundIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <div className="flex min-w-0 flex-wrap items-center gap-2">
                    <span className="min-w-0 break-all font-medium text-sm">{r.label}</span>
                    <Badge variant={r.kind === 'recovery' ? 'warning' : 'secondary'}>
                      {KIND_LABEL[r.kind]}
                    </Badge>
                  </div>
                  <code
                    className="truncate font-mono text-muted-foreground text-xs"
                    title={r.publicKey}
                  >
                    {r.publicKey}
                  </code>
                </div>
                <span className="hidden text-muted-foreground text-xs sm:inline">
                  {r.addedAt ? `added ${relativeTime(r.addedAt)}` : ''}
                </span>
                {r.kind !== 'passphrase' ? (
                  <Button
                    aria-label={`Remove ${r.label}`}
                    size="icon-sm"
                    variant="ghost-destructive"
                    onClick={() => setRemoving(r)}
                  >
                    <Trash2Icon />
                  </Button>
                ) : (
                  <span className="w-8" />
                )}
              </div>
            ))
          )}
        </SettingsSection>
      </PageContainer>
      <AddRecipientDialog existing={list} open={adding} onOpenChange={setAdding} />
      <ConfirmDialog
        confirmLabel="Remove recipient"
        open={removing !== null}
        pending={remove.isPending}
        title={`Remove ${removing?.label ?? ''}?`}
        onConfirm={() => removing && remove.mutate({ label: removing.label })}
        onOpenChange={(o) => !o && setRemoving(null)}
      >
        <Alert variant="warning">
          <AlertTitle>History stays readable to them</AlertTitle>
          <AlertDescription>
            They lose access to future versions only. Everything already in git history remains
            decryptable with their key — rotate the vault key and change exposed secrets if that
            matters.
          </AlertDescription>
        </Alert>
      </ConfirmDialog>
      <ConfirmDialog
        confirmLabel="Rotate key"
        description="Generates a new vault key and signing key, re-encrypts every entry, profile, manifest and session archive, and commits once. Other devices must pull before writing."
        open={rotating}
        pending={rotate.isPending}
        title="Rotate the vault key?"
        onConfirm={() => rotate.mutate(undefined)}
        onOpenChange={setRotating}
      />
    </>
  );
}
