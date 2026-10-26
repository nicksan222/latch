import { LatchAgent } from '@latch/rpc/v1';
import type { ProfileSummary } from '@latch/rpc/v1';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { ContextMenu, ContextMenuPopup, ContextMenuTrigger } from '@latch/ui/context-menu';
import { LatchMark } from '@latch/ui/latch-mark';
import { Menu, MenuItem, MenuPopup, MenuSeparator, MenuTrigger } from '@latch/ui/menu';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail
} from '@latch/ui/sidebar';
import { Skeleton } from '@latch/ui/skeleton';
import { Tooltip, TooltipPopup, TooltipTrigger } from '@latch/ui/tooltip';
import { Link, useNavigate, useRouterState } from '@tanstack/react-router';
import {
  ArrowDownUpIcon,
  CheckIcon,
  EllipsisIcon,
  FileInputIcon,
  GlobeIcon,
  KeyRoundIcon,
  PencilIcon,
  PlusIcon,
  SettingsIcon,
  Trash2Icon,
  TriangleAlertIcon,
  UsersIcon
} from 'lucide-react';
import { useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { ProfileAvatar } from '~/components/icons';
import { qk, useProfiles, useRpcMutation, useSyncStatus, useVaultStatus } from '~/lib/api/queries';
import { notify } from '~/lib/notify';
import { basename } from '~/lib/profiles';

function ProfileActions({
  profile,
  onDelete,
  kind
}: {
  profile: ProfileSummary;
  onDelete: () => void;
  kind: 'menu' | 'context';
}) {
  const navigate = useNavigate();
  const launch = useRpcMutation(LatchAgent.method.browserLaunch, {
    invalidate: [qk.browser, qk.profiles],
    onSuccess: () => notify.success(`Browser launched for ${profile.name}`),
    onError: (error) => notify.error('Could not launch browser', error)
  });
  const close = useRpcMutation(LatchAgent.method.browserClose, {
    invalidate: [qk.browser, qk.profiles],
    onError: (error) => notify.error('Could not close browser', error)
  });
  const Item = MenuItem;
  const items = (
    <>
      <Item
        onClick={() =>
          void navigate({ to: '/profile/$profileId', params: { profileId: profile.id } })
        }
      >
        <PencilIcon /> Edit profile
      </Item>
      {profile.browserRunning ? (
        <Item onClick={() => close.mutate({ id: profile.id })}>
          <GlobeIcon /> Close browser
        </Item>
      ) : (
        <Item onClick={() => launch.mutate({ profileId: profile.id, url: undefined })}>
          <GlobeIcon /> Launch browser
        </Item>
      )}
      <MenuSeparator />
      <Item variant="destructive" onClick={onDelete}>
        <Trash2Icon /> Delete profile…
      </Item>
    </>
  );
  if (kind === 'context') return <ContextMenuPopup>{items}</ContextMenuPopup>;
  return <MenuPopup align="start">{items}</MenuPopup>;
}

function ProfileRow({ profile, active }: { profile: ProfileSummary; active: boolean }) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const navigate = useNavigate();
  const remove = useRpcMutation(LatchAgent.method.profilesDelete, {
    invalidate: [qk.profiles, qk.entries],
    onSuccess: () => {
      notify.success(
        `Deleted profile ${profile.name}`,
        'Entries remain recoverable from Trash for 30 days.'
      );
      setConfirmDelete(false);
      void navigate({ to: '/' });
    },
    onError: (error) => notify.error('Could not delete profile', error)
  });
  return (
    <SidebarMenuItem>
      <ContextMenu>
        <ContextMenuTrigger>
          <SidebarMenuButton
            isActive={active}
            render={<Link params={{ profileId: profile.id }} to="/p/$profileId" />}
            className="pe-14"
            data-testid={`profile-${profile.name}`}
          >
            <ProfileAvatar color={profile.color} icon={profile.icon} size="sm" />
            <span className="min-w-0 flex-1 truncate">{profile.name}</span>
          </SidebarMenuButton>
        </ContextMenuTrigger>
        <ProfileActions kind="context" profile={profile} onDelete={() => setConfirmDelete(true)} />
      </ContextMenu>
      <div className="pointer-events-none absolute inset-y-0 end-1.5 flex items-center gap-1">
        {profile.browserRunning ? (
          <Tooltip>
            <TooltipTrigger
              render={
                <span
                  aria-label="Browser running"
                  className="pointer-events-auto size-1.5 rounded-full bg-success motion-safe:animate-status-pulse"
                  role="img"
                />
              }
            />
            <TooltipPopup side="right">Browser running</TooltipPopup>
          </Tooltip>
        ) : null}
        <span className="text-2xs tabular-nums text-sidebar-muted-foreground group-hover/menu-item:hidden group-focus-within/menu-item:hidden">
          {profile.entryCount}
        </span>
        <Menu>
          <MenuTrigger
            render={
              <Button
                aria-label={`Actions for ${profile.name}`}
                className="pointer-events-auto hidden group-hover/menu-item:inline-flex group-focus-within/menu-item:inline-flex data-popup-open:inline-flex"
                size="icon-micro"
                variant="ghost-muted"
              />
            }
          >
            <EllipsisIcon />
          </MenuTrigger>
          <ProfileActions kind="menu" profile={profile} onDelete={() => setConfirmDelete(true)} />
        </Menu>
      </div>
      <ConfirmDialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title={`Delete “${profile.name}”?`}
        description={`This deletes the profile and its ${profile.entryCount} entries. They stay recoverable from git history (Trash) for 30 days.`}
        confirmLabel="Delete profile"
        pending={remove.isPending}
        onConfirm={() => remove.mutate({ id: profile.id })}
      />
    </SidebarMenuItem>
  );
}

function SyncBadge() {
  const sync = useSyncStatus();
  const s = sync.data;
  if (!s) return null;
  if (s.conflicts.length > 0)
    return (
      <Badge variant="warning" size="sm" aria-label={`${s.conflicts.length} conflicts`}>
        <TriangleAlertIcon /> {s.conflicts.length}
      </Badge>
    );
  if (s.ahead || s.behind)
    return (
      <span
        className="text-2xs tabular-nums text-sidebar-muted-foreground"
        aria-label={`${s.ahead} ahead, ${s.behind} behind`}
      >
        ↑{s.ahead} ↓{s.behind}
      </span>
    );
  return s.hasRemote ? <CheckIcon aria-label="In sync" className="size-3.5 text-success" /> : null;
}

const NAV = [
  { to: '/sync', label: 'Sync', icon: ArrowDownUpIcon },
  { to: '/recipients', label: 'Recipients', icon: UsersIcon },
  { to: '/import-export', label: 'Import / Export', icon: FileInputIcon },
  { to: '/trash', label: 'Trash', icon: Trash2Icon },
  { to: '/settings', label: 'Settings', icon: SettingsIcon }
] as const;

export function AppSidebar() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const profiles = useProfiles();
  const status = useVaultStatus();
  const activeProfileId = /^\/(?:p|profile)\/([^/]+)/.exec(pathname)?.[1];

  return (
    <Sidebar
      collapsible="offcanvas"
      data-app-sidebar=""
      resizable={{ storageKey: 'latch:sidebar-width', minWidth: 220, maxWidth: 400 }}
    >
      <SidebarHeader className="h-(--workspace-topbar-height) flex-row items-center gap-2 px-3">
        <span className="inline-flex size-6 items-center justify-center rounded-md bg-primary text-primary-foreground">
          <LatchMark className="size-4" />
        </span>
        <div className="flex min-w-0 flex-col leading-tight">
          <span className="font-semibold text-sm tracking-tight">Latch</span>
          {status.data?.path ? (
            <span
              className="truncate text-2xs text-sidebar-muted-foreground"
              title={status.data.path}
            >
              {basename(status.data.path)}
            </span>
          ) : null}
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <div className="flex h-7 items-center justify-between ps-(--sidebar-row-content-inset) pe-1">
            <span className="font-medium text-2xs text-sidebar-muted-foreground uppercase tracking-wider">
              Profiles
            </span>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    aria-label="New profile"
                    render={<Link to="/profiles/new" />}
                    size="icon-micro"
                    variant="ghost-muted"
                  />
                }
              >
                <PlusIcon />
              </TooltipTrigger>
              <TooltipPopup side="right">New profile</TooltipPopup>
            </Tooltip>
          </div>
          <SidebarMenu aria-label="Profiles">
            {profiles.isPending
              ? Array.from({ length: 3 }, (_, i) => (
                  <Skeleton className="mx-1 h-7 rounded-md" key={i} />
                ))
              : profiles.data?.map((p) => (
                  <ProfileRow active={p.id === activeProfileId} key={p.id} profile={p} />
                ))}
            {profiles.data?.length === 0 ? (
              <SidebarMenuItem>
                <SidebarMenuButton render={<Link to="/profiles/new" />}>
                  <PlusIcon /> <span>Create a profile</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ) : null}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="border-t border-sidebar-border/60 py-2">
        <SidebarMenu aria-label="Vault">
          {NAV.map((item) => (
            <SidebarMenuItem key={item.to}>
              <SidebarMenuButton isActive={pathname === item.to} render={<Link to={item.to} />}>
                <item.icon />
                <span className="flex-1">{item.label}</span>
                {item.to === '/sync' ? <SyncBadge /> : null}
              </SidebarMenuButton>
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
        <div className="flex items-center gap-1.5 px-(--sidebar-row-content-inset) pt-1 text-2xs text-sidebar-muted-foreground">
          <KeyRoundIcon className="size-3" aria-hidden />
          <span className="truncate font-mono">{status.data?.vaultId?.slice(0, 12) ?? ''}</span>
        </div>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
