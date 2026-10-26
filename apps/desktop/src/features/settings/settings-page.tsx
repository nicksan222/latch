import { Theme } from '@latch/rpc/v1';
import { VaultTrustState, Severity } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { DoctorReport, Settings, TrustStatus } from '@latch/rpc/v1';
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
import { Switch } from '@latch/ui/switch';
import { ToggleGroup, ToggleGroupItem } from '@latch/ui/toggle-group';
import { useQueryClient } from '@tanstack/react-query';
import {
  CircleAlertIcon,
  InfoIcon,
  KeyRoundIcon,
  MonitorIcon,
  MoonIcon,
  StethoscopeIcon,
  SunIcon,
  TriangleAlertIcon,
  WrenchIcon
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { FormField, PasswordInput } from '~/components/fields';
import { PageContainer, PageTitle, SettingsRow, SettingsSection } from '~/components/page';
import { SimpleSelect } from '~/components/simple-select';
import { StrengthMeter } from '~/components/strength-meter';
import { AppHeader } from '~/features/shell/app-header';
import { call } from '~/lib/api/client';
import { errorMessage } from '~/lib/api/errors';
import { qk, useSettings, useVaultStatus } from '~/lib/api/queries';
import { notify } from '~/lib/notify';
import { useStrength } from '~/lib/strength';
import { useThemeStore } from '~/stores/theme';

const secs = (values: number[], zero = 'Never') =>
  values.map((v) => ({
    value: String(v),
    label:
      v === 0
        ? zero
        : v < 60
          ? `${v} seconds`
          : v < 3600
            ? `${v / 60} minute${v === 60 ? '' : 's'}`
            : `${v / 3600} hour${v === 3600 ? '' : 's'}`
  }));

const AUTO_LOCK = secs([60, 300, 900, 1800, 3600, 14400, 0]);
const CLIPBOARD = secs([10, 20, 30, 45, 60, 120, 0]);
const REVEAL = secs([5, 10, 20, 30, 60]);

function ChangePassphraseDialog({
  open,
  onOpenChange
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const reset = () => {
    setCurrent('');
    setNext('');
    setConfirm('');
    setError(null);
  };
  const strong = (useStrength(next)?.value?.score ?? -1) >= 2;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o);
        if (!o) reset();
      }}
    >
      <DialogPopup>
        <form
          className="contents"
          onSubmit={async (e) => {
            e.preventDefault();
            setPending(true);
            setError(null);
            try {
              await call(LatchAgent.method.vaultChangePassphrase, { current, newPassphrase: next });
              notify.success('Passphrase changed', 'Only vaultkey.age was re-encrypted.');
              reset();
              onOpenChange(false);
            } catch (err) {
              setError(errorMessage(err));
            } finally {
              setPending(false);
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>Change passphrase</DialogTitle>
            <DialogDescription>
              Other devices keep using the old passphrase until they pull.
            </DialogDescription>
          </DialogHeader>
          <DialogPanel className="flex flex-col gap-4">
            <FormField error={error ?? undefined} label="Current passphrase">
              {(a) => (
                <PasswordInput
                  {...a}
                  autoComplete="current-password"
                  value={current}
                  onChange={setCurrent}
                />
              )}
            </FormField>
            <FormField label="New passphrase">
              {(a) => (
                <PasswordInput {...a} autoComplete="new-password" value={next} onChange={setNext} />
              )}
            </FormField>
            <StrengthMeter passphrase={next} />
            <FormField
              error={confirm && confirm !== next ? 'Passphrases do not match' : undefined}
              label="Confirm new passphrase"
            >
              {(a) => (
                <PasswordInput
                  {...a}
                  autoComplete="new-password"
                  value={confirm}
                  onChange={setConfirm}
                />
              )}
            </FormField>
          </DialogPanel>
          <DialogFooter>
            <Button variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={!current || !strong || next !== confirm || pending} type="submit">
              {pending ? <Spinner /> : null} Change passphrase
            </Button>
          </DialogFooter>
        </form>
      </DialogPopup>
    </Dialog>
  );
}

const TRUST_LABEL: Record<TrustStatus['state'], string> = {
  [VaultTrustState.UNSPECIFIED]: 'Unknown',
  [VaultTrustState.FIRST_USE]: 'First use',
  [VaultTrustState.PINNED]: 'Pinned',
  [VaultTrustState.STALE]: 'Stale'
};

function Fingerprint({ trust }: { trust: TrustStatus | null | undefined }) {
  return (
    <SettingsRow
      control={
        trust ? (
          <Badge variant={trust.state === VaultTrustState.PINNED ? 'success' : 'warning'}>
            {TRUST_LABEL[trust.state]}
          </Badge>
        ) : null
      }
      description="Identifies this vault's keys. Compare it with your other devices and the recovery kit; a different value means a different (possibly forged) vault."
      title="Vault fingerprint"
    >
      <code className="break-all font-mono text-sm" data-testid="vault-fingerprint">
        {trust?.fingerprint ?? '—'}
      </code>
    </SettingsRow>
  );
}

function Doctor() {
  const queryClient = useQueryClient();
  const [report, setReport] = useState<DoctorReport | null>(null);
  const [pending, setPending] = useState<'run' | 'fix' | null>(null);
  useEffect(() => {
    if (window.location.hash === '#doctor')
      document.getElementById('doctor')?.scrollIntoView({ block: 'center' });
  }, []);
  const run = async (fix: boolean) => {
    setPending(fix ? 'fix' : 'run');
    try {
      const r = await call(LatchAgent.method.doctorRun, {
        fix,
        rebuildManifest: fix,
        staged: false
      });
      setReport(r);
      if (fix) void queryClient.invalidateQueries({ queryKey: qk.status });
      if (fix && r.fixed.length)
        notify.success(`Fixed ${r.fixed.length} issue${r.fixed.length === 1 ? '' : 's'}`);
    } catch (e) {
      notify.error('Doctor failed', e);
    } finally {
      setPending(null);
    }
  };
  const fixable = report?.issues.some((i) => i.severity !== Severity.INFO) ?? false;
  return (
    <div id="doctor">
      <SettingsRow
        control={
          <div className="flex gap-2">
            <Button
              disabled={pending !== null}
              size="sm"
              variant="outline"
              onClick={() => void run(false)}
            >
              {pending === 'run' ? <Spinner /> : <StethoscopeIcon />} Run doctor
            </Button>
            {fixable ? (
              <Button disabled={pending !== null} size="sm" onClick={() => void run(true)}>
                {pending === 'fix' ? <Spinner /> : <WrenchIcon />} Fix
              </Button>
            ) : null}
          </div>
        }
        description="Checks for plaintext in the repo, manifest/signature integrity, and repository hygiene."
        title="Doctor"
      >
        {report ? (
          <ul className="flex flex-col gap-1.5" data-testid="doctor-report">
            {report.ok && report.issues.length === 0 ? (
              <li className="text-success-foreground text-sm">No issues found.</li>
            ) : null}
            {report.fixed.map((f) => (
              <li className="flex items-center gap-2 text-sm text-success-foreground" key={f}>
                <WrenchIcon className="size-3.5" aria-hidden /> {f}
              </li>
            ))}
            {report.issues.map((i) => (
              <li key={i.code + (i.path ?? '')}>
                <Alert
                  variant={
                    i.severity === Severity.ERROR
                      ? 'error'
                      : i.severity === Severity.WARNING
                        ? 'warning'
                        : 'info'
                  }
                >
                  {i.severity === Severity.ERROR ? (
                    <CircleAlertIcon aria-hidden />
                  ) : i.severity === Severity.WARNING ? (
                    <TriangleAlertIcon aria-hidden />
                  ) : (
                    <InfoIcon aria-hidden />
                  )}
                  <AlertTitle className="flex flex-wrap items-center justify-between gap-2">
                    {i.message}
                    <Badge
                      variant={
                        i.severity === Severity.ERROR
                          ? 'error'
                          : i.severity === Severity.WARNING
                            ? 'warning'
                            : 'info'
                      }
                    >
                      {i.severity}
                    </Badge>
                  </AlertTitle>
                  <AlertDescription className="break-all font-mono text-xs">
                    {i.code}
                    {i.path ? ` · ${i.path}` : ''}
                  </AlertDescription>
                </Alert>
              </li>
            ))}
          </ul>
        ) : null}
      </SettingsRow>
    </div>
  );
}

function GitIdentity({
  settings,
  save
}: {
  settings: Settings;
  save: (patch: Partial<Settings>) => void;
}) {
  const [name, setName] = useState(settings.gitName ?? '');
  const [email, setEmail] = useState(settings.gitEmail ?? '');
  const dirty = name !== (settings.gitName ?? '') || email !== (settings.gitEmail ?? '');
  return (
    <SettingsRow description="Author of the commits Latch creates." title="Git identity">
      <div className="flex flex-wrap items-end gap-2">
        <Input
          aria-label="Git name"
          className="w-44"
          placeholder="Name"
          size="sm"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
        />
        <Input
          aria-label="Git email"
          className="w-56"
          placeholder="email@example.com"
          size="sm"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.currentTarget.value)}
        />
        <Button
          disabled={!dirty}
          size="sm"
          variant="outline"
          onClick={() =>
            save({ gitName: name.trim() || undefined, gitEmail: email.trim() || undefined })
          }
        >
          Save
        </Button>
      </div>
    </SettingsRow>
  );
}

export function SettingsPage() {
  const settings = useSettings();
  const status = useVaultStatus();
  const queryClient = useQueryClient();
  const setTheme = useThemeStore((s) => s.setTheme);
  const theme = useThemeStore((s) => s.theme);
  const [changePass, setChangePass] = useState(false);
  const [confirmSessions, setConfirmSessions] = useState(false);
  const [shortcut, setShortcut] = useState<string | null>(null);

  const save = async (patch: Partial<Settings>) => {
    const current = queryClient.getQueryData<Settings>(qk.settings) ?? settings.data;
    if (!current) return;
    const next = { ...current, ...patch };
    queryClient.setQueryData(qk.settings, next);
    try {
      const saved = await call(LatchAgent.method.settingsUpdate, next);
      queryClient.setQueryData(qk.settings, saved);
      void queryClient.invalidateQueries({ queryKey: qk.status });
    } catch (e) {
      notify.error('Could not save settings', e);
      void queryClient.invalidateQueries({ queryKey: qk.settings });
    }
  };

  const s = settings.data;
  return (
    <>
      <AppHeader title={<PageTitle>Settings</PageTitle>} />
      <PageContainer>
        {!s ? (
          <div className="flex flex-col gap-3">
            <Skeleton className="h-40 rounded-xl" />
            <Skeleton className="h-40 rounded-xl" />
          </div>
        ) : (
          <>
            <SettingsSection title="Appearance">
              <SettingsRow
                control={
                  <ToggleGroup
                    aria-label="Theme"
                    value={[String(theme)]}
                    onValueChange={(v) => {
                      const t = v[0] === undefined ? undefined : (Number(v[0]) as Theme);
                      if (!t) return;
                      setTheme(t);
                      void save({ theme: t });
                    }}
                  >
                    <ToggleGroupItem aria-label="System theme" value={String(Theme.SYSTEM)}>
                      <MonitorIcon /> System
                    </ToggleGroupItem>
                    <ToggleGroupItem aria-label="Light theme" value={String(Theme.LIGHT)}>
                      <SunIcon /> Light
                    </ToggleGroupItem>
                    <ToggleGroupItem aria-label="Dark theme" value={String(Theme.DARK)}>
                      <MoonIcon /> Dark
                    </ToggleGroupItem>
                  </ToggleGroup>
                }
                description="Follows the OS by default."
                title="Theme"
              />
            </SettingsSection>

            <SettingsSection title="Security">
              <SettingsRow
                control={
                  <SimpleSelect
                    aria-label="Auto-lock"
                    className="w-40"
                    options={AUTO_LOCK}
                    size="sm"
                    value={String(s.autoLockSecs)}
                    onValueChange={(v) => void save({ autoLockSecs: Number(v) })}
                  />
                }
                description="Lock after this much inactivity."
                title="Auto-lock"
              />
              <SettingsRow
                control={
                  <SimpleSelect
                    aria-label="Clipboard clear"
                    className="w-40"
                    options={CLIPBOARD}
                    size="sm"
                    value={String(s.clipboardClearSecs)}
                    onValueChange={(v) => void save({ clipboardClearSecs: Number(v) })}
                  />
                }
                description="Copied secrets are cleared after this delay, if still on the clipboard."
                title="Clear clipboard"
              />
              <SettingsRow
                control={
                  <SimpleSelect
                    aria-label="Reveal timeout"
                    className="w-40"
                    options={REVEAL}
                    size="sm"
                    value={String(s.revealHideSecs)}
                    onValueChange={(v) => void save({ revealHideSecs: Number(v) })}
                  />
                }
                description="Revealed values hide again after this delay or when you navigate away."
                title="Hide revealed values"
              />
              <SettingsRow
                control={
                  <Switch
                    aria-label="Lock on screen lock"
                    checked={s.lockOnScreenLock}
                    onCheckedChange={(v) => void save({ lockOnScreenLock: v })}
                  />
                }
                title="Lock when the screen locks"
              />
              <SettingsRow
                control={
                  <Button size="sm" variant="outline" onClick={() => setChangePass(true)}>
                    <KeyRoundIcon /> Change…
                  </Button>
                }
                title="Master passphrase"
              />
              <Fingerprint trust={status.data?.trust} />
            </SettingsSection>

            <SettingsSection title="Desktop">
              <SettingsRow
                control={
                  <Switch
                    aria-label="Tray icon"
                    checked={s.trayEnabled}
                    onCheckedChange={(v) => void save({ trayEnabled: v })}
                  />
                }
                description="Lock/unlock, open browsers and copy pinned TOTP codes from the tray."
                title="Tray icon"
              />
              <SettingsRow
                control={
                  <Switch
                    aria-label="Start at login"
                    checked={s.autostart}
                    onCheckedChange={(v) => void save({ autostart: v })}
                  />
                }
                title="Start at login"
              />
              <SettingsRow
                control={
                  <Input
                    aria-label="Global shortcut"
                    className="w-52 font-mono"
                    size="sm"
                    value={shortcut ?? s.globalShortcut}
                    onBlur={() => {
                      if (shortcut !== null && shortcut.trim() && shortcut !== s.globalShortcut)
                        void save({ globalShortcut: shortcut.trim() });
                      setShortcut(null);
                    }}
                    onChange={(e) => setShortcut(e.currentTarget.value)}
                  />
                }
                description="Opens quick search from anywhere (e.g. CmdOrCtrl+Shift+L)."
                title="Global shortcut"
              />
            </SettingsSection>

            <SettingsSection title="Git & sync (this vault)">
              <SettingsRow
                control={
                  <Switch
                    aria-label="Entry names in commit messages"
                    checked={s.namesInCommits}
                    onCheckedChange={(v) => void save({ namesInCommits: v })}
                  />
                }
                description="Off by default: commit messages use ids so a public repo does not reveal which services you use."
                title="Entry names in commit messages"
              />
              <SettingsRow
                control={
                  <Switch
                    aria-label="Auto push"
                    checked={s.autoPush}
                    onCheckedChange={(v) => void save({ autoPush: v })}
                  />
                }
                description="Push after every change."
                title="Auto push"
              />
              <SettingsRow
                control={
                  <Switch
                    aria-label="Commit browser sessions"
                    checked={s.sessionsCommit}
                    onCheckedChange={(v) =>
                      v ? setConfirmSessions(true) : void save({ sessionsCommit: false })
                    }
                  />
                }
                description="Store encrypted browser session archives in the repository so they sync across machines."
                title="Commit browser sessions"
              />
              <GitIdentity
                key={`${s.gitName}|${s.gitEmail}`}
                save={(p) => void save(p)}
                settings={s}
              />
            </SettingsSection>

            <SettingsSection title="Maintenance">
              <Doctor />
            </SettingsSection>
          </>
        )}
      </PageContainer>
      <ChangePassphraseDialog open={changePass} onOpenChange={setChangePass} />
      <ConfirmDialog
        confirmLabel="Commit sessions"
        description="Session archives can be large and every change adds a new blob to git history. Consider git LFS once an archive exceeds 20 MiB."
        destructive={false}
        open={confirmSessions}
        title="Commit browser sessions to git?"
        onConfirm={() => {
          void save({ sessionsCommit: true });
          setConfirmSessions(false);
        }}
        onOpenChange={setConfirmSessions}
      />
    </>
  );
}
