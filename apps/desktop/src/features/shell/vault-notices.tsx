import { Alert, AlertAction, AlertDescription, AlertTitle } from '@latch/ui/alert';
import { Button } from '@latch/ui/button';
import { useNavigate } from '@tanstack/react-router';
import { FingerprintIcon, ShieldAlertIcon, XIcon } from 'lucide-react';
import { useState } from 'react';
import { useVaultStatus } from '~/lib/api/queries';
import type { VaultStatus } from '../../_generated/contracts';

/**
 * Non-blocking banners for the unlocked shell:
 * - first use of this vault on this machine: show the fingerprint so the
 *   user can compare it on another device / the recovery kit (dismissible);
 * - integrity problems or an interrupted rotation: link to the doctor.
 */
export function VaultNoticesView({
  status,
  onOpenDoctor
}: {
  status: VaultStatus | undefined;
  onOpenDoctor: () => void;
}) {
  const [dismissed, setDismissed] = useState<string | null>(null);
  if (!status || status.locked) return null;
  const trust = status.trust;
  const firstUse = trust?.state === 'first_use' && dismissed !== trust.fingerprint;
  const problems = status.integrityProblems;
  const needsDoctor = problems.length > 0 || status.rotationPending;
  if (!firstUse && !needsDoctor) return null;
  return (
    <div className="flex flex-col gap-2 px-3 pt-3 sm:px-4" data-testid="vault-notices">
      {needsDoctor ? <IntegrityNotice problems={problems} onOpenDoctor={onOpenDoctor} /> : null}
      {firstUse && trust ? (
        <Alert data-testid="trust-banner" variant="info">
          <FingerprintIcon />
          <AlertTitle>First unlock of this vault on this device</AlertTitle>
          <AlertDescription>
            <span>
              Vault fingerprint{' '}
              <code className="font-mono text-foreground">{trust.fingerprint}</code>. Compare it on
              another device (Settings → Security) or with your recovery kit. If it differs, do not
              use this copy.
            </span>
          </AlertDescription>
          <AlertAction>
            <Button
              aria-label="Dismiss fingerprint notice"
              size="icon-sm"
              variant="ghost"
              onClick={() => setDismissed(trust.fingerprint)}
            >
              <XIcon />
            </Button>
          </AlertAction>
        </Alert>
      ) : null}
    </div>
  );
}

function IntegrityNotice({
  problems,
  onOpenDoctor
}: {
  problems: string[];
  onOpenDoctor: () => void;
}) {
  return (
    <Alert data-testid="integrity-banner" variant="error">
      <ShieldAlertIcon />
      <AlertTitle>
        {problems.length > 0 ? 'Vault integrity problem' : 'Key rotation not finished'}
      </AlertTitle>
      <AlertDescription>
        {problems.length > 0 ? (
          <ul className="list-disc ps-4">
            {problems.slice(0, 3).map((p) => (
              <li key={p}>{p}</li>
            ))}
            {problems.length > 3 ? <li>…and {problems.length - 3} more</li> : null}
          </ul>
        ) : (
          <span>An interrupted key rotation must be completed.</span>
        )}
        <span>Data access is blocked until the doctor repairs the vault.</span>
      </AlertDescription>
      <AlertAction>
        <Button size="sm" onClick={onOpenDoctor}>
          Open doctor
        </Button>
      </AlertAction>
    </Alert>
  );
}

export function VaultNotices() {
  const status = useVaultStatus();
  const navigate = useNavigate();
  return (
    <VaultNoticesView
      status={status.data}
      onOpenDoctor={() => void navigate({ to: '/settings', hash: 'doctor' })}
    />
  );
}
