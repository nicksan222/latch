import { ErrorCode } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { VaultStatus } from '@latch/rpc/v1';
import { Alert, AlertDescription } from '@latch/ui/alert';
import { Button } from '@latch/ui/button';
import { Input } from '@latch/ui/input';
import { LatchMark } from '@latch/ui/latch-mark';
import { Spinner } from '@latch/ui/spinner';
import { StandalonePage } from '@latch/ui/standalone-page';
import { cn } from '@latch/ui/utils';
import { useQueryClient } from '@tanstack/react-query';
import { FileKeyIcon, FolderOpenIcon, KeyRoundIcon, PlusIcon } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { FormField, PasswordInput } from '~/components/fields';
import { SimpleSelect } from '~/components/simple-select';
import { call } from '~/lib/api/client';
import { errorMessage, isRpcError } from '~/lib/api/errors';
import { useKnownVaults } from '~/lib/api/queries';
import { isTauri } from '~/lib/api/transport';
import { pickPath } from '~/lib/clipboard';
import { notify } from '~/lib/notify';
import { useUiStore } from '~/stores/ui';

export function LockScreen({ status }: { status: VaultStatus }) {
  const queryClient = useQueryClient();
  const vaults = useKnownVaults();
  const setAddingVault = useUiStore((s) => s.setAddingVault);
  const [mode, setMode] = useState<'passphrase' | 'identity'>('passphrase');
  const [passphrase, setPassphrase] = useState('');
  const [identityFile, setIdentityFile] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [shake, setShake] = useState(0);
  const [pending, setPending] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, [mode, status.path]);

  const unlock = async () => {
    setPending(true);
    setError(null);
    try {
      await call(
        LatchAgent.method.vaultUnlock,
        mode === 'passphrase'
          ? { passphrase, identityFile: undefined, timeoutSecs: undefined }
          : { passphrase: undefined, identityFile, timeoutSecs: undefined }
      );
      setPassphrase('');
      await queryClient.invalidateQueries();
    } catch (err) {
      setError(
        isRpcError(err, ErrorCode.BAD_CREDENTIALS)
          ? errorMessage(err) || 'Wrong passphrase'
          : errorMessage(err)
      );
      setShake((n) => n + 1);
      setPassphrase('');
      inputRef.current?.focus();
    } finally {
      setPending(false);
    }
  };

  const switchVault = async (path: string) => {
    try {
      await call(LatchAgent.method.vaultOpen, { path });
      setError(null);
      await queryClient.invalidateQueries();
    } catch (err) {
      notify.error('Could not open vault', err);
    }
  };

  const canSubmit = mode === 'passphrase' ? passphrase.length > 0 : identityFile.trim().length > 0;

  return (
    <StandalonePage tone="brand">
      <form
        className="flex flex-col gap-6"
        data-testid="lock-screen"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSubmit && !pending) void unlock();
        }}
      >
        <div className="flex items-center gap-3">
          <span className="inline-flex size-11 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-primary/24 shadow-sm">
            <LatchMark className="size-5" />
          </span>
          <div className="min-w-0">
            <h1 className="font-semibold text-xl tracking-tight">Latch is locked</h1>
            <p className="truncate text-muted-foreground text-sm" title={status.path ?? undefined}>
              {status.path}
            </p>
          </div>
        </div>

        <div className={cn('flex flex-col gap-2', shake > 0 && 'animate-shake')} key={shake}>
          {mode === 'passphrase' ? (
            <FormField error={error ?? undefined} label="Passphrase">
              {(a) => (
                <PasswordInput
                  {...a}
                  autoComplete="current-password"
                  ref={inputRef}
                  value={passphrase}
                  onChange={setPassphrase}
                />
              )}
            </FormField>
          ) : (
            <FormField
              description="An age identity (recovery kit or YubiKey plugin) that is a vault recipient."
              error={error ?? undefined}
              label="Identity file"
            >
              {(a) => (
                <div className="flex gap-2">
                  <Input
                    {...a}
                    className="font-mono"
                    placeholder="~/.config/latch/identity.txt"
                    ref={inputRef}
                    value={identityFile}
                    onChange={(e) => setIdentityFile(e.currentTarget.value)}
                  />
                  {isTauri() ? (
                    <Button
                      aria-label="Browse for identity file"
                      size="icon"
                      variant="outline"
                      onClick={async () => {
                        const p = await pickPath({ title: 'Choose an age identity' });
                        if (p) setIdentityFile(p);
                      }}
                    >
                      <FolderOpenIcon />
                    </Button>
                  ) : null}
                </div>
              )}
            </FormField>
          )}
        </div>

        <div className="flex flex-col gap-2">
          <Button disabled={!canSubmit || pending} size="lg" type="submit">
            {pending ? <Spinner /> : <KeyRoundIcon />} Unlock
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              setMode(mode === 'passphrase' ? 'identity' : 'passphrase');
              setError(null);
            }}
          >
            <FileKeyIcon />{' '}
            {mode === 'passphrase' ? 'Use an identity file instead' : 'Use passphrase instead'}
          </Button>
        </div>

        <div className="flex flex-col gap-3 border-t pt-5">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-muted-foreground text-xs">Vault</span>
            <SimpleSelect
              aria-label="Switch vault"
              className="min-w-0 flex-1"
              options={(vaults.data ?? []).map((v) => ({ value: v.path, label: v.path }))}
              size="sm"
              value={status.path ?? null}
              onValueChange={(path) => path !== status.path && void switchVault(path)}
            />
            <Button size="sm" variant="outline" onClick={() => setAddingVault(true)}>
              <PlusIcon /> Add vault
            </Button>
          </div>
          <p className="truncate font-mono text-2xs text-muted-foreground">id {status.vaultId}</p>
        </div>
        {status.recipients.length > 0 ? null : (
          <Alert variant="warning">
            <AlertDescription>This vault has no recipients configured.</AlertDescription>
          </Alert>
        )}
      </form>
    </StandalonePage>
  );
}
