import { create } from '@bufbuild/protobuf';
import {
  BookmarkSchema,
  BrowserSettingsSchema,
  ProfileInputSchema,
  type BrowserSettings as BrowserSettingsMessage
} from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { ProfileInput } from '@latch/rpc/v1';
import { Alert, AlertDescription } from '@latch/ui/alert';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { Input } from '@latch/ui/input';
import { RadioGroup, Radio } from '@latch/ui/radio-group';
import { Skeleton } from '@latch/ui/skeleton';
import { Spinner } from '@latch/ui/spinner';
import { Switch } from '@latch/ui/switch';
import { Textarea } from '@latch/ui/textarea';
import { cn } from '@latch/ui/utils';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useParams } from '@tanstack/react-router';
import { GlobeIcon, PlusIcon, RotateCcwIcon, SaveIcon, SquareIcon, Trash2Icon } from 'lucide-react';
import { useId, useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { FormField } from '~/components/fields';
import { PROFILE_ICONS, ProfileAvatar } from '~/components/icons';
import { PageContainer, PageTitle, SettingsRow, SettingsSection } from '~/components/page';
import { SimpleSelect } from '~/components/simple-select';
import { AppHeader } from '~/features/shell/app-header';
import { useKeyedRows } from '~/hooks/use-keyed-rows';
import {
  qk,
  useBrowserSessions,
  useProfile,
  useRpcMutation,
  useSessionInfo
} from '~/lib/api/queries';
import { useAsyncAction } from '~/lib/async-ui';
import { formatBytes } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { defaultBrowserSettings } from '~/lib/profiles';
import { relativeTime } from '~/lib/time';
import { validateProfileInput, type FieldErrors } from '~/lib/validation';

/** Profile colors offered in the picker (profile colors are user data, not theme). */
export const PROFILE_COLORS = [
  '#2563eb',
  '#16a34a',
  '#db2777',
  '#ea580c',
  '#7c3aed',
  '#0891b2',
  '#ca8a04',
  '#dc2626',
  '#4b5563'
];
function BrowserPanel({ profileId, profileName }: { profileId: string; profileName: string }) {
  const queryClient = useQueryClient();
  const sessions = useBrowserSessions();
  const info = useSessionInfo(profileId);
  const [confirmReset, setConfirmReset] = useState(false);
  const session = sessions.data?.find((s) => s.profileId === profileId);
  const invalidate = [qk.browser, qk.profiles];
  const launch = useRpcMutation(LatchAgent.method.browserLaunch, {
    invalidate,
    onError: (e) => notify.error('Could not launch', e)
  });
  const close = useRpcMutation(LatchAgent.method.browserClose, {
    invalidate,
    onSuccess: () => notify.success('Browser closed', 'Session archived and encrypted.'),
    onError: (e) => notify.error('Could not close', e)
  });
  const reset = useRpcMutation(LatchAgent.method.browserReset, {
    invalidate,
    onSuccess: () => {
      notify.success('Session reset', 'Cookies and local storage were discarded.');
      setConfirmReset(false);
    },
    onError: (e) => notify.error('Could not reset session', e)
  });
  return (
    <>
      <SettingsSection
        title="Browser session"
        headerAction={
          <Badge variant={session ? 'success' : 'secondary'}>
            {session ? 'Running' : 'Stopped'}
          </Badge>
        }
      >
        <SettingsRow
          control={
            session ? (
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant="outline"
                  disabled={launch.isPending}
                  onClick={() => launch.mutate({ profileId, url: undefined })}
                >
                  {launch.isPending ? 'Preparing browser…' : 'Open window'}
                </Button>
                <Button
                  size="sm"
                  variant="destructive-outline"
                  onClick={() => close.mutate({ id: profileId })}
                >
                  <SquareIcon /> Close
                </Button>
              </div>
            ) : (
              <Button
                disabled={launch.isPending}
                size="sm"
                onClick={() => launch.mutate({ profileId, url: undefined })}
              >
                {launch.isPending ? <Spinner /> : <GlobeIcon />}{' '}
                {launch.isPending ? 'Preparing browser…' : 'Launch browser'}
              </Button>
            )
          }
          description={
            session
              ? `pid ${session.pid} · started ${relativeTime(session.startedAt)} · ${session.userDataDir}`
              : `An isolated browser whose cookies and storage belong to ${profileName} only.`
          }
          title="Isolated browser"
        />
        <SettingsRow
          control={
            <Button
              disabled={!info.data?.exists || Boolean(session)}
              size="sm"
              variant="outline"
              onClick={() => setConfirmReset(true)}
            >
              <RotateCcwIcon /> Reset session
            </Button>
          }
          description={<SessionArchiveDescription info={info} />}
          title="Encrypted session archive"
        />
      </SettingsSection>
      <ConfirmDialog
        confirmLabel="Reset session"
        description="Discards the encrypted browser session archive (cookies, local storage). You will be signed out of every site in this profile's browser."
        open={confirmReset}
        pending={reset.isPending}
        title={`Reset ${profileName}'s browser session?`}
        onConfirm={() => {
          reset.mutate({ id: profileId });
          void queryClient.invalidateQueries({ queryKey: qk.sessionInfo(profileId) });
        }}
        onOpenChange={setConfirmReset}
      />
    </>
  );
}

function ListEditor({
  items,
  onChange,
  placeholder,
  label,
  errors,
  prefix
}: {
  items: string[];
  onChange: (items: string[]) => void;
  placeholder: string;
  label: string;
  errors: FieldErrors;
  prefix: string;
}) {
  const keyed = useKeyedRows(items, onChange);
  return (
    <div className="flex flex-col gap-2">
      {keyed.rows.map(({ id, value: v }, i) => (
        <FormField error={errors[`${prefix}.${i}`]} key={id} label={`${label} ${i + 1}`}>
          {(a) => (
            <div className="flex gap-2">
              <Input
                {...a}
                placeholder={placeholder}
                size="sm"
                value={v}
                onChange={(e) => keyed.update(i, e.currentTarget.value)}
              />
              <Button
                aria-label={`Remove ${label} ${i + 1}`}
                size="icon-sm"
                variant="ghost-destructive"
                onClick={() => keyed.remove(i)}
              >
                <Trash2Icon />
              </Button>
            </div>
          )}
        </FormField>
      ))}
      <Button className="self-start" size="xs" variant="ghost" onClick={() => keyed.append('')}>
        <PlusIcon /> Add {label.toLowerCase()}
      </Button>
    </div>
  );
}

function ProfileForm({ initial, profileId }: { initial: ProfileInput; profileId: string | null }) {
  const navigate = useNavigate();
  const [value, setValue] = useState<ProfileInput>(initial);
  const validation = useAsyncAction(value);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [confirmDelete, setConfirmDelete] = useState(false);
  const b = value.browser ?? defaultBrowserSettings();
  const setBrowser = (patch: Partial<typeof b>) =>
    setValue({ ...value, browser: { ...b, ...patch } });
  const create = useRpcMutation(LatchAgent.method.profilesCreate, {
    invalidate: [qk.profiles],
    onSuccess: (p) => {
      notify.success(`Created ${p.name}`);
      void navigate({ to: '/p/$profileId', params: { profileId: p.id } });
    },
    onError: (e) => notify.error('Could not create profile', e)
  });
  const update = useRpcMutation(LatchAgent.method.profilesUpdate, {
    invalidate: [qk.profiles],
    onSuccess: (p) => notify.success(`Saved ${p.name}`),
    onError: (e) => notify.error('Could not save profile', e)
  });
  const remove = useRpcMutation(LatchAgent.method.profilesDelete, {
    invalidate: [qk.profiles, qk.entries],
    onSuccess: () => {
      notify.success('Profile deleted');
      void navigate({ to: '/' });
    },
    onError: (e) => notify.error('Could not delete profile', e)
  });
  const save = () => {
    void validation.run(
      () => validateProfileInput(value),
      (errs) => {
        setErrors(errs);
        if (Object.keys(errs).length) return;
        if (profileId) update.mutate({ id: profileId, input: value });
        else create.mutate(value);
      },
      (error) => notify.error('Could not validate profile', error)
    );
  };
  const pending = create.isPending || update.isPending || validation.pending;

  return (
    <>
      <AppHeader
        title={
          <div className="flex min-w-0 items-center gap-2">
            <ProfileAvatar color={value.color} icon={value.icon} size="md" />
            <PageTitle>{profileId ? `${initial.name} · settings` : 'New profile'}</PageTitle>
          </div>
        }
        actions={
          <Button disabled={pending} size="sm" onClick={save}>
            {pending ? <Spinner /> : <SaveIcon />} {profileId ? 'Save' : 'Create profile'}
          </Button>
        }
      />
      <PageContainer>
        <SettingsSection title="Profile">
          <div className="grid gap-4 p-4 sm:grid-cols-2">
            <FormField error={errors.name} label="Name">
              {(a) => (
                <Input
                  {...a}
                  autoFocus={!profileId}
                  value={value.name}
                  onChange={(e) => setValue({ ...value, name: e.currentTarget.value })}
                />
              )}
            </FormField>
            <FormField label="Icon">
              {(a) => (
                <SimpleSelect
                  id={a.id}
                  options={Object.keys(PROFILE_ICONS).map((k) => {
                    const Icon = PROFILE_ICONS[k]!;
                    return {
                      value: k,
                      label: (
                        <span className="inline-flex items-center gap-2">
                          <Icon className="size-4" aria-hidden /> {k}
                        </span>
                      )
                    };
                  })}
                  value={value.icon}
                  onValueChange={(icon) => setValue({ ...value, icon })}
                />
              )}
            </FormField>
            <div className="flex flex-col gap-1.5 sm:col-span-2">
              <span className="font-medium text-xs" id="color-label">
                Color
              </span>
              <RadioGroup
                aria-labelledby="color-label"
                className="flex flex-row flex-wrap gap-2"
                value={value.color}
                onValueChange={(color) => setValue({ ...value, color: color as string })}
              >
                {PROFILE_COLORS.map((c) => (
                  <Radio
                    aria-label={c}
                    className={cn(
                      'size-7 rounded-full border-2 border-transparent bg-(--profile-color) data-checked:border-foreground data-checked:ring-2 data-checked:ring-inset data-checked:ring-background sm:size-7 dark:not-data-checked:bg-(--profile-color) [&>*]:hidden'
                    )}
                    key={c}
                    style={{ '--profile-color': c } as React.CSSProperties}
                    value={c}
                  />
                ))}
              </RadioGroup>
            </div>
            <FormField className="sm:col-span-2" label="Description">
              {(a) => (
                <Textarea
                  {...a}
                  rows={2}
                  value={value.description}
                  onChange={(e) => setValue({ ...value, description: e.currentTarget.value })}
                />
              )}
            </FormField>
          </div>
          <SettingsRow
            control={
              <Switch
                aria-label="Pinned"
                checked={value.pinned}
                onCheckedChange={(pinned) => setValue({ ...value, pinned })}
              />
            }
            description="Pinned profiles are listed first."
            title="Pinned"
          />
        </SettingsSection>

        <BrowserSettings b={b} setBrowser={setBrowser} errors={errors} />

        {profileId ? <BrowserPanel profileId={profileId} profileName={initial.name} /> : null}

        {profileId ? (
          <SettingsSection title="Danger zone">
            <SettingsRow
              control={
                <Button
                  size="sm"
                  variant="destructive-outline"
                  onClick={() => setConfirmDelete(true)}
                >
                  <Trash2Icon /> Delete profile
                </Button>
              }
              description="Deletes the profile and all of its entries. Recoverable from git history for 30 days."
              title="Delete profile"
            />
          </SettingsSection>
        ) : (
          <Alert variant="info">
            <AlertDescription>
              Browser sessions become available after the profile is created.
            </AlertDescription>
          </Alert>
        )}
        <ConfirmDialog
          confirmLabel="Delete profile"
          description="The profile, its entries and its browser session archive are deleted."
          open={confirmDelete}
          pending={remove.isPending}
          title={`Delete “${initial.name}”?`}
          onConfirm={() => profileId && remove.mutate({ id: profileId })}
          onOpenChange={setConfirmDelete}
        />
      </PageContainer>
    </>
  );
}

export function ProfileEditor({ mode }: { mode: 'create' | 'edit' }) {
  const params = useParams({ strict: false }) as { profileId?: string };
  const profileId = mode === 'edit' ? params.profileId : undefined;
  const profile = useProfile(profileId);
  if (mode === 'create') {
    return (
      <ProfileForm
        initial={create(ProfileInputSchema, {
          name: '',
          color: PROFILE_COLORS[0]!,
          icon: 'briefcase',
          description: '',
          pinned: false,
          browser: defaultBrowserSettings()
        })}
        profileId={null}
      />
    );
  }
  if (profile.isPending)
    return (
      <PageContainer>
        <Skeleton className="h-40" />
      </PageContainer>
    );
  if (!profile.data) return <PageContainer>Profile not found.</PageContainer>;
  const p = profile.data;
  return (
    <ProfileForm
      initial={create(ProfileInputSchema, {
        name: p.name,
        color: p.color,
        icon: p.icon,
        description: p.description,
        pinned: p.pinned,
        browser: create(BrowserSettingsSchema, {
          ...(p.browser ?? defaultBrowserSettings())
        })
      })}
      key={p.id + p.updatedAt}
      profileId={p.id}
    />
  );
}

function BookmarkEditor({
  bookmarks,
  errors,
  onChange
}: {
  bookmarks: BrowserSettingsMessage['bookmarks'];
  errors: FieldErrors;
  onChange: (bookmarks: BrowserSettingsMessage['bookmarks']) => void;
}) {
  const bookmarkErrorId = useId();
  const keyed = useKeyedRows(bookmarks, onChange);
  return (
    <div className="flex flex-col gap-2">
      {keyed.rows.map(({ id, value: bm }, i) => (
        <div className="flex flex-col gap-1" key={id}>
          <div className="flex gap-2">
            <Input
              aria-label={`Bookmark ${i + 1} title`}
              aria-invalid={errors[`bookmarks.${i}`] ? true : undefined}
              aria-describedby={errors[`bookmarks.${i}`] ? `${bookmarkErrorId}-${i}` : undefined}
              className="w-40"
              placeholder="Title"
              size="sm"
              value={bm.title}
              onChange={(e) => keyed.update(i, { ...bm, title: e.currentTarget.value })}
            />
            <Input
              aria-label={`Bookmark ${i + 1} URL`}
              aria-invalid={errors[`bookmarks.${i}`] ? true : undefined}
              aria-describedby={errors[`bookmarks.${i}`] ? `${bookmarkErrorId}-${i}` : undefined}
              placeholder="https://"
              size="sm"
              value={bm.url}
              onChange={(e) => keyed.update(i, { ...bm, url: e.currentTarget.value })}
            />
            <Button
              aria-label={`Remove bookmark ${i + 1}`}
              size="icon-sm"
              variant="ghost-destructive"
              onClick={() => keyed.remove(i)}
            >
              <Trash2Icon />
            </Button>
          </div>
          {errors[`bookmarks.${i}`] ? (
            <Alert id={`${bookmarkErrorId}-${i}`} variant="error">
              <AlertDescription>{errors[`bookmarks.${i}`]}</AlertDescription>
            </Alert>
          ) : null}
        </div>
      ))}
      <Button
        className="self-start"
        size="xs"
        variant="ghost"
        onClick={() => keyed.append(create(BookmarkSchema))}
      >
        <PlusIcon /> Add bookmark
      </Button>
    </div>
  );
}

function BrowserSettings({
  b,
  setBrowser,
  errors
}: {
  b: BrowserSettingsMessage;
  setBrowser: (patch: Partial<BrowserSettingsMessage>) => void;
  errors: FieldErrors;
}) {
  return (
    <SettingsSection title="Browser">
      <SettingsRow
        control={<Badge variant="secondary">Firefox</Badge>}
        description="Latch-managed Firefox is downloaded for isolated profile sessions. First launch may take a few minutes."
        title="Provider"
      />
      <SettingsRow description="Opened when the browser launches." title="Start pages">
        <ListEditor
          errors={errors}
          items={b.startPages}
          label="Start page"
          placeholder="https://"
          prefix="browser.startPages"
          onChange={(startPages) => setBrowser({ startPages })}
        />
      </SettingsRow>
      <SettingsRow description="Written into the browser's bookmark bar." title="Bookmarks">
        <BookmarkEditor
          bookmarks={b.bookmarks}
          errors={errors}
          onChange={(bookmarks) => setBrowser({ bookmarks })}
        />
      </SettingsRow>
    </SettingsSection>
  );
}

function SessionArchiveDescription({ info }: { info: ReturnType<typeof useSessionInfo> }) {
  return info.isPending ? (
    <Skeleton className="h-3 w-40" />
  ) : info.data?.exists ? (
    `${formatBytes(Number(info.data.sizeBytes))} · updated ${relativeTime(info.data.updatedAt)} · ${info.data.committed ? 'committed to the vault repo' : 'local cache only'}`
  ) : (
    'No saved session yet. It is archived and encrypted when the browser closes.'
  );
}
