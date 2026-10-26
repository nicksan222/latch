import { LatchAgent } from '@latch/rpc/v1';
import type { EntryInput, EntryType } from '@latch/rpc/v1';
import { Button } from '@latch/ui/button';
import { Spinner } from '@latch/ui/spinner';
import { useNavigate, useParams } from '@tanstack/react-router';
import { SaveIcon } from 'lucide-react';
import { useState } from 'react';
import { EntryTypeIcon } from '~/components/icons';
import { ContentTitle, PageContent } from '~/components/page';
import { qk, useRpcMutation } from '~/lib/api/queries';
import { useAsyncAction } from '~/lib/async-ui';
import { ENTRY_TYPE_LABEL, emptyEntryInput } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { validateEntryInput, type FieldErrors } from '~/lib/validation';
import { EntryForm } from './entry-form';

export function NewEntry() {
  const { profileId, type } = useParams({ from: '/p/$profileId/new/$type' });
  return <NewEntryForm key={`${profileId}:${type}`} profileId={profileId} type={type} />;
}

function NewEntryForm({ profileId, type }: { profileId: string; type: EntryType }) {
  const navigate = useNavigate();
  const [draft, setDraft] = useState<EntryInput>(() => emptyEntryInput(profileId, type));
  const [errors, setErrors] = useState<FieldErrors>({});
  const validation = useAsyncAction(draft);
  const create = useRpcMutation(LatchAgent.method.entriesCreate, {
    invalidate: [qk.entries, qk.profiles],
    onSuccess: (entry) => {
      notify.success(`Created ${entry.name}`);
      void navigate({
        to: '/p/$profileId/e/$entryId',
        params: { profileId: entry.profileId, entryId: entry.id }
      });
    },
    onError: (e) => notify.error('Could not create entry', e)
  });
  const submit = () => {
    void validation.run(
      () => validateEntryInput(draft),
      (errs) => {
        setErrors(errs);
        if (Object.keys(errs).length === 0) create.mutate(draft);
      },
      (error) => notify.error('Could not validate entry', error)
    );
  };
  return (
    <form
      className="contents"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <PageContent>
        <div className="flex flex-wrap items-center gap-3">
          <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-lg border bg-card text-muted-foreground">
            <EntryTypeIcon className="size-5" type={type} />
          </span>
          <ContentTitle className="flex-1">New {ENTRY_TYPE_LABEL[type].toLowerCase()}</ContentTitle>
          <Button
            variant="ghost"
            onClick={() => void navigate({ to: '/p/$profileId', params: { profileId } })}
          >
            Cancel
          </Button>
          <Button disabled={create.isPending || validation.pending} type="submit">
            {create.isPending ? <Spinner /> : <SaveIcon />} Save
          </Button>
        </div>
        <EntryForm errors={errors} value={draft} onChange={setDraft} />
      </PageContent>
    </form>
  );
}
