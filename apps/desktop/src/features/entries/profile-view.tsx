import { Button } from '@latch/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@latch/ui/empty';
import { Menu, MenuItem, MenuPopup, MenuTrigger } from '@latch/ui/menu';
import { Spinner } from '@latch/ui/spinner';
import { Tooltip, TooltipPopup, TooltipTrigger } from '@latch/ui/tooltip';
import { Link, Navigate, Outlet, useNavigate, useParams } from '@tanstack/react-router';
import { GlobeIcon, KeySquareIcon, PlusIcon, Settings2Icon, SquareIcon } from 'lucide-react';
import { EntryTypeIcon, ProfileAvatar } from '~/components/icons';
import { PageTitle } from '~/components/page';
import { ActionsButton } from '~/features/actions/actions-button';
import { AppHeader } from '~/features/shell/app-header';
import { qk, useProfiles, useRpcMutation } from '~/lib/api/queries';
import { ENTRY_TYPES, ENTRY_TYPE_LABEL } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { EntryList } from './entry-list';

export function NewEntryMenu({ profileId }: { profileId: string }) {
  const navigate = useNavigate();
  return (
    <Menu>
      <MenuTrigger render={<Button aria-label="New entry" size="sm" className="px-2 lg:px-2.5" />}>
        <PlusIcon /> <span className="hidden lg:inline">New entry</span>
      </MenuTrigger>
      <MenuPopup align="end">
        {ENTRY_TYPES.map((t) => (
          <MenuItem
            key={t}
            onClick={() =>
              void navigate({ to: '/p/$profileId/new/$type', params: { profileId, type: t } })
            }
          >
            <EntryTypeIcon type={t} /> {ENTRY_TYPE_LABEL[t]}
          </MenuItem>
        ))}
      </MenuPopup>
    </Menu>
  );
}

export function BrowserButton({
  profileId,
  running,
  name
}: {
  profileId: string;
  running: boolean;
  name: string;
}) {
  const launch = useRpcMutation('browser.launch', {
    invalidate: [qk.browser, qk.profiles],
    onSuccess: (s) => notify.success(`Browser launched for ${name}`, `${s.engine}, pid ${s.pid}`),
    onError: (e) => notify.error('Could not launch browser', e)
  });
  const close = useRpcMutation('browser.close', {
    invalidate: [qk.browser, qk.profiles],
    onSuccess: () => notify.success('Browser closed', 'Session archived and encrypted.'),
    onError: (e) => notify.error('Could not close browser', e)
  });
  const pending = launch.isPending || close.isPending;
  return running ? (
    <Button
      aria-label="Close browser"
      size="sm"
      variant="outline"
      className="px-2 lg:px-2.5"
      disabled={pending}
      onClick={() => close.mutate({ id: profileId })}
    >
      {pending ? <Spinner /> : <SquareIcon />}{' '}
      <span className="hidden lg:inline">Close browser</span>
    </Button>
  ) : (
    <Button
      aria-label="Browser"
      size="sm"
      variant="outline"
      className="px-2 lg:px-2.5"
      disabled={pending}
      onClick={() => launch.mutate({ profileId, url: null, engine: null })}
    >
      {pending ? <Spinner /> : <GlobeIcon />} <span className="hidden lg:inline">Browser</span>
    </Button>
  );
}

export function ProfileView() {
  const { profileId } = useParams({ from: '/p/$profileId' });
  const profiles = useProfiles();
  const profile = profiles.data?.find((p) => p.id === profileId);

  if (profiles.isSuccess && !profile) return <Navigate replace to="/" />;

  return (
    <>
      <AppHeader
        title={
          profile ? (
            <>
              <ProfileAvatar color={profile.color} icon={profile.icon} size="md" />
              <div className="flex min-w-0 flex-col leading-tight">
                <PageTitle>{profile.name}</PageTitle>
                {profile.description ? (
                  <span className="truncate text-muted-foreground text-xs">
                    {profile.description}
                  </span>
                ) : null}
              </div>
            </>
          ) : (
            <Spinner tone="muted" />
          )
        }
        actions={
          profile ? (
            <>
              <ActionsButton key={profileId} profileId={profileId} />
              <BrowserButton
                name={profile.name}
                profileId={profile.id}
                running={profile.browserRunning}
              />
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      aria-label="Profile settings"
                      render={<Link params={{ profileId }} to="/profile/$profileId" />}
                      size="icon-sm"
                      variant="ghost"
                    />
                  }
                >
                  <Settings2Icon />
                </TooltipTrigger>
                <TooltipPopup>Profile & browser settings</TooltipPopup>
              </Tooltip>
              <NewEntryMenu profileId={profileId} />
            </>
          ) : null
        }
      />
      <div className="flex min-h-0 flex-1">
        <EntryList
          className="w-64 shrink-0 border-r border-border/60 lg:w-80 xl:w-88"
          key={profileId}
          profileId={profileId}
        />
        <div className="flex min-w-0 flex-1 flex-col overflow-y-auto">
          <Outlet />
        </div>
      </div>
    </>
  );
}

export function NoEntrySelected() {
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <KeySquareIcon />
        </EmptyMedia>
        <EmptyTitle>Select an entry</EmptyTitle>
        <EmptyDescription>
          Pick an entry on the left, or press Ctrl/⌘ K to search every profile.
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
