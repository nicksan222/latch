import { Button } from '@latch/ui/button';
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle
} from '@latch/ui/empty';
import { Spinner } from '@latch/ui/spinner';
import { Link, Navigate } from '@tanstack/react-router';
import { FolderPlusIcon } from 'lucide-react';
import { PageTitle } from '~/components/page';
import { useProfiles } from '~/lib/api/queries';
import { AppHeader } from './app-header';

/** `/`: jump to the first profile, or invite to create one. */
export function HomePage() {
  const profiles = useProfiles();
  const first = profiles.data?.[0];
  if (first) return <Navigate params={{ profileId: first.id }} replace to="/p/$profileId" />;
  return (
    <>
      <AppHeader title={<PageTitle>Latch</PageTitle>} />
      {profiles.isPending ? (
        <div className="flex flex-1 items-center justify-center">
          <Spinner tone="muted" />
        </div>
      ) : (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FolderPlusIcon />
            </EmptyMedia>
            <EmptyTitle>Create your first profile</EmptyTitle>
            <EmptyDescription>
              Profiles group secrets per project, client or persona — each with its own isolated
              browser.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button render={<Link to="/profiles/new" />}>New profile</Button>
          </EmptyContent>
        </Empty>
      )}
    </>
  );
}
