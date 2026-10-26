import { LatchAgent } from '@latch/rpc/v1';
import type { KdfBenchmark, RecoveryKit } from '@latch/rpc/v1';
import { Alert, AlertDescription, AlertTitle } from '@latch/ui/alert';
import { Button } from '@latch/ui/button';
import { Checkbox } from '@latch/ui/checkbox';
import { useCopyToClipboard } from '@latch/ui/hooks/use-copy-to-clipboard';
import { Input } from '@latch/ui/input';
import { LatchMark } from '@latch/ui/latch-mark';
import { Progress, ProgressIndicator, ProgressTrack } from '@latch/ui/progress';
import { Spinner } from '@latch/ui/spinner';
import { StandalonePage } from '@latch/ui/standalone-page';
import { Switch } from '@latch/ui/switch';
import { cn } from '@latch/ui/utils';
import { WizardSteps } from '@latch/ui/wizard';
import { useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeftIcon,
  CheckIcon,
  ClipboardCopyIcon,
  CloudDownloadIcon,
  CpuIcon,
  DownloadIcon,
  FolderOpenIcon,
  SparklesIcon
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { FormField, PasswordInput } from '~/components/fields';
import { StrengthMeter } from '~/components/strength-meter';
import { call } from '~/lib/api/client';
import { errorMessage } from '~/lib/api/errors';
import { useVaultStatus } from '~/lib/api/queries';
import { isTauri } from '~/lib/api/transport';
import { pickPath } from '~/lib/clipboard';
import { notify } from '~/lib/notify';
import { useStrength } from '~/lib/strength';
import { useUiStore } from '~/stores/ui';

const STEPS = ['Location', 'Passphrase', 'Identity', 'Recovery kit'] as const;

function PathField({
  label,
  value,
  onChange,
  directory = true
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  directory?: boolean;
}) {
  return (
    <FormField
      description={
        isTauri() ? undefined : 'Path on this machine (the development fixture keeps it in memory).'
      }
      label={label}
    >
      {(a) => (
        <div className="flex gap-2">
          <Input
            {...a}
            className="font-mono"
            value={value}
            onChange={(e) => onChange(e.currentTarget.value)}
          />
          {isTauri() ? (
            <Button
              aria-label="Browse"
              size="icon"
              variant="outline"
              onClick={async () => {
                const p = await pickPath({ directory, title: label });
                if (p) onChange(p);
              }}
            >
              <FolderOpenIcon />
            </Button>
          ) : null}
        </div>
      )}
    </FormField>
  );
}

function KdfBenchmarkPanel({
  result,
  onResult
}: {
  result: KdfBenchmark | null;
  onResult: (r: KdfBenchmark) => void;
}) {
  const [progress, setProgress] = useState(result ? 100 : 5);
  const [running, setRunning] = useState(!result);
  // Keep one request across Strict Mode's effect replay; the RPC itself is not cancellable.
  const request = useRef<Promise<KdfBenchmark> | null>(null);
  useEffect(() => {
    if (result) return;
    let active = true;
    const t0 = Date.now();
    const timer = setInterval(
      () => setProgress(Math.min(92, 5 + ((Date.now() - t0) / 1000) * 90)),
      80
    );
    request.current ??= call(LatchAgent.method.vaultBenchmarkKdf, { targetMs: 1000 });
    void request.current.then(
      (benchmark) => {
        if (!active) return;
        clearInterval(timer);
        onResult(benchmark);
        setProgress(100);
        setRunning(false);
      },
      (error: unknown) => {
        if (!active) return;
        clearInterval(timer);
        notify.error('Benchmark failed', error);
        setRunning(false);
      }
    );
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [onResult, result]);
  return (
    <div className="flex flex-col gap-2 rounded-xl border p-3" data-testid="kdf-benchmark">
      <div className="flex items-center gap-2 text-sm">
        <CpuIcon aria-hidden className="size-4 text-muted-foreground" />
        <span className="font-medium">Argon2id calibration</span>
        {result ? <CheckIcon aria-label="Done" className="ms-auto size-4 text-success" /> : null}
      </div>
      <Progress aria-label="Argon2 benchmark progress" value={result ? 100 : progress}>
        <ProgressTrack>
          <ProgressIndicator />
        </ProgressTrack>
      </Progress>
      <p className="text-muted-foreground text-xs">
        {result?.params
          ? `${Math.round(result.params.memoryKib / 1024)} MiB · ${result.params.iterations} iterations · ${result.params.parallelism} lanes ≈ ${(result.measuredMs / 1000).toFixed(1)} s per unlock`
          : running
            ? 'Measuring how hard this machine can make brute-forcing your passphrase…'
            : 'Not run yet.'}
      </p>
    </div>
  );
}

function RecoveryKitStep({ kit, onDone }: { kit: RecoveryKit; onDone: () => void }) {
  const [saved, setSaved] = useState(false);
  const { copyToClipboard, isCopied } = useCopyToClipboard({ target: 'recovery kit' });
  const download = () => {
    const url = URL.createObjectURL(new Blob([kit.document], { type: 'text/markdown' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = `latch-recovery-${kit.vaultId.slice(0, 8)}.md`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  return (
    <div className="flex flex-col gap-4" data-testid="recovery-kit">
      <Alert variant="warning">
        <AlertTitle>Shown once</AlertTitle>
        <AlertDescription>
          Latch never stores this kit. Anyone holding the recovery key can decrypt the vault — keep
          it offline. Without the passphrase or this key the vault is unrecoverable.
        </AlertDescription>
      </Alert>
      <pre className="max-h-64 overflow-auto rounded-xl border bg-muted/50 p-3 font-mono text-xs leading-relaxed">
        {kit.document}
      </pre>
      <div className="flex flex-wrap gap-2">
        <Button variant="outline" onClick={() => copyToClipboard(kit.document, undefined)}>
          <ClipboardCopyIcon /> {isCopied ? 'Copied' : 'Copy'}
        </Button>
        <Button variant="outline" onClick={download}>
          <DownloadIcon /> Save as file…
        </Button>
      </div>
      <label className="flex items-center gap-2 text-sm">
        <Checkbox checked={saved} onCheckedChange={(v) => setSaved(v === true)} />I saved my
        recovery kit somewhere safe
      </label>
      <Button disabled={!saved} size="lg" onClick={onDone}>
        Open Latch
      </Button>
    </div>
  );
}

function CreateWizard({ onBack }: { onBack: () => void }) {
  const queryClient = useQueryClient();
  const setAddingVault = useUiStore((s) => s.setAddingVault);
  const status = useVaultStatus();
  const [step, setStep] = useState(0);
  const [path, setPath] = useState(status.data?.initialized ? '~/latch-vault-2' : '~/latch-vault');
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [bench, setBench] = useState<KdfBenchmark | null>(null);
  const [gitName, setGitName] = useState('');
  const [gitEmail, setGitEmail] = useState('');
  const [remoteUrl, setRemoteUrl] = useState('');
  const [withKit, setWithKit] = useState(true);
  const [kit, setKit] = useState<RecoveryKit | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const strength = useStrength(passphrase)?.value;
  const mismatch = confirm.length > 0 && confirm !== passphrase;
  const canNext = [
    path.trim().length > 0,
    (strength?.score ?? -1) >= 2 && passphrase === confirm && bench !== null,
    true
  ][step];

  const finish = async () => {
    setAddingVault(false);
    await queryClient.invalidateQueries();
  };

  const create = async () => {
    setPending(true);
    setError(null);
    // Keep onboarding mounted until the one-time kit has been acknowledged.
    setAddingVault(true);
    try {
      const result = await call(LatchAgent.method.vaultInit, {
        path: path.trim(),
        passphrase,
        kdf: bench?.params,
        gitName: gitName.trim() || undefined,
        gitEmail: gitEmail.trim() || undefined,
        remoteUrl: remoteUrl.trim() || undefined,
        recoveryKit: withKit
      });
      setPassphrase('');
      setConfirm('');
      if (result.recoveryKit) {
        setKit(result.recoveryKit);
        setStep(3);
      } else {
        await finish();
      }
    } catch (e) {
      setError(errorMessage(e));
      if (!status.data?.initialized) setAddingVault(false);
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <WizardSteps currentStep={step} steps={withKit ? STEPS : STEPS.slice(0, 3)} />
      {step === 0 ? (
        <div className="flex flex-col gap-4">
          <PathField label="Vault folder" value={path} onChange={setPath} />
          <p className="text-muted-foreground text-xs">
            A git repository is created here. Only ciphertext and public config are ever committed —
            safe to push to a public remote.
          </p>
        </div>
      ) : null}
      {step === 1 ? (
        <div className="flex flex-col gap-4">
          <FormField label="Master passphrase">
            {(a) => (
              <PasswordInput
                {...a}
                autoComplete="new-password"
                generate={false}
                value={passphrase}
                onChange={setPassphrase}
              />
            )}
          </FormField>
          <StrengthMeter passphrase={passphrase} />
          <FormField
            error={mismatch ? 'Passphrases do not match' : undefined}
            label="Confirm passphrase"
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
          <KdfBenchmarkPanel result={bench} onResult={setBench} />
        </div>
      ) : null}
      {step === 2 ? (
        <div className="flex flex-col gap-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <FormField description="Author of vault commits" label="Git name">
              {(a) => (
                <Input {...a} value={gitName} onChange={(e) => setGitName(e.currentTarget.value)} />
              )}
            </FormField>
            <FormField label="Git email">
              {(a) => (
                <Input
                  {...a}
                  type="email"
                  value={gitEmail}
                  onChange={(e) => setGitEmail(e.currentTarget.value)}
                />
              )}
            </FormField>
          </div>
          <FormField description="Optional. You can add it later in Sync." label="Remote URL">
            {(a) => (
              <Input
                {...a}
                className="font-mono"
                placeholder="git@github.com:you/vault.git"
                value={remoteUrl}
                onChange={(e) => setRemoteUrl(e.currentTarget.value)}
              />
            )}
          </FormField>
          <label
            aria-label="Generate a recovery kit"
            className="flex items-start gap-3 rounded-xl border p-3 text-sm"
          >
            <Switch checked={withKit} onCheckedChange={setWithKit} />
            <span className="flex flex-col gap-0.5">
              <span className="font-medium">Generate a recovery kit</span>
              <span className="text-muted-foreground text-xs">
                Adds a recovery age identity as a recipient. Strongly recommended: a lost passphrase
                is otherwise unrecoverable.
              </span>
            </span>
          </label>
          {error ? (
            <Alert variant="error">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}
        </div>
      ) : null}
      {step === 3 && kit ? <RecoveryKitStep kit={kit} onDone={() => void finish()} /> : null}

      {step < 3 ? (
        <CreateWizardNavigation
          step={step}
          pending={pending}
          canNext={Boolean(canNext)}
          onBack={onBack}
          setStep={setStep}
          create={create}
        />
      ) : null}
    </div>
  );
}

function CreateWizardNavigation({
  step,
  pending,
  canNext,
  onBack,
  setStep,
  create
}: {
  step: number;
  pending: boolean;
  canNext: boolean;
  onBack: () => void;
  setStep: (step: number) => void;
  create: () => Promise<void>;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <Button variant="ghost" onClick={() => (step === 0 ? onBack() : setStep(step - 1))}>
        <ArrowLeftIcon /> Back
      </Button>
      {step < 2 ? (
        <Button disabled={!canNext} onClick={() => setStep(step + 1)}>
          Continue
        </Button>
      ) : (
        <Button disabled={pending} onClick={() => void create()}>
          {pending ? <Spinner /> : <SparklesIcon />} Create vault
        </Button>
      )}
    </div>
  );
}

function CloneForm({ onBack }: { onBack: () => void }) {
  const queryClient = useQueryClient();
  const setAddingVault = useUiStore((s) => s.setAddingVault);
  const [url, setUrl] = useState('');
  const [path, setPath] = useState('~/latch-vault');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={async (e) => {
        e.preventDefault();
        setPending(true);
        setError(null);
        try {
          await call(LatchAgent.method.vaultClone, { url: url.trim(), path: path.trim() });
          notify.success('Vault cloned', 'Unlock it with your passphrase or identity.');
          setAddingVault(false);
          await queryClient.invalidateQueries();
        } catch (err) {
          setError(errorMessage(err));
        } finally {
          setPending(false);
        }
      }}
    >
      <FormField label="Repository URL">
        {(a) => (
          <Input
            {...a}
            autoFocus
            className="font-mono"
            placeholder="git@github.com:you/vault.git"
            value={url}
            onChange={(e) => setUrl(e.currentTarget.value)}
          />
        )}
      </FormField>
      <PathField label="Clone into" value={path} onChange={setPath} />
      {error ? (
        <Alert variant="error">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}
      <div className="flex items-center justify-between">
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeftIcon /> Back
        </Button>
        <Button disabled={!url.trim() || !path.trim() || pending} type="submit">
          {pending ? <Spinner /> : <CloudDownloadIcon />} Clone vault
        </Button>
      </div>
    </form>
  );
}

export function Onboarding() {
  const [mode, setMode] = useState<'choose' | 'create' | 'clone'>('choose');
  const status = useVaultStatus();
  const setAddingVault = useUiStore((s) => s.setAddingVault);
  return (
    <StandalonePage tone="brand">
      <div className="flex flex-col gap-6" data-testid="onboarding">
        <div className="flex items-center gap-3">
          <span className="inline-flex size-11 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-primary/24 shadow-sm">
            <LatchMark className="size-5" />
          </span>
          <div className="min-w-0">
            <p className="font-semibold text-xs text-muted-foreground uppercase tracking-[0.18em]">
              Latch
            </p>
            <h1 className="font-semibold text-2xl tracking-tight">
              {mode === 'clone'
                ? 'Clone a vault'
                : mode === 'create'
                  ? 'Create a vault'
                  : 'Welcome'}
            </h1>
          </div>
        </div>
        {mode === 'choose' ? (
          <div className="flex flex-col gap-4">
            <p className="text-muted-foreground text-sm leading-relaxed">
              A local, git-backed secret manager. Your vault is an encrypted git repository — no
              server, no account.
            </p>
            <div className="grid gap-3 sm:grid-cols-2">
              {[
                {
                  key: 'create' as const,
                  icon: SparklesIcon,
                  title: 'Create a new vault',
                  text: 'Start fresh on this machine.'
                },
                {
                  key: 'clone' as const,
                  icon: CloudDownloadIcon,
                  title: 'Clone a vault',
                  text: 'Bring an existing vault from a git remote.'
                }
              ].map((o) => (
                <button
                  className={cn(
                    'flex flex-col items-start gap-2 rounded-xl border bg-card p-4 text-left outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring'
                  )}
                  key={o.key}
                  type="button"
                  onClick={() => setMode(o.key)}
                >
                  <o.icon aria-hidden className="size-5 text-primary" />
                  <span className="font-medium text-sm">{o.title}</span>
                  <span className="text-muted-foreground text-xs">{o.text}</span>
                </button>
              ))}
            </div>
            {status.data?.initialized ? (
              <Button className="self-start" variant="ghost" onClick={() => setAddingVault(false)}>
                <ArrowLeftIcon /> Back to lock screen
              </Button>
            ) : null}
          </div>
        ) : mode === 'create' ? (
          <CreateWizard onBack={() => setMode('choose')} />
        ) : (
          <CloneForm onBack={() => setMode('choose')} />
        )}
      </div>
    </StandalonePage>
  );
}
