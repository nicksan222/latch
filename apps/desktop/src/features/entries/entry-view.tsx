import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { Spinner } from '@latch/ui/spinner';
import { DownloadIcon, ExternalLinkIcon } from 'lucide-react';
import { useState } from 'react';
import { PlainField } from '~/components/fields';
import { SecretField, TotpField } from '~/components/secret-field';
import { useCopyField } from '~/components/use-copy-field';
import { EnvEditor } from '~/features/env/env-editor';
import { call } from '~/lib/api/client';
import { writeClipboardText } from '~/lib/clipboard';
import { formatBytes } from '~/lib/entries';
import { notify } from '~/lib/notify';
import type { Entry, EntryPayload } from '../../_generated/contracts';

function Grid({ children }: { children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-1 gap-4 @min-[28rem]/page-content:grid-cols-2">{children}</div>
  );
}

function copyPublic(label: string, value: string) {
  void writeClipboardText(value).then(
    () => notify.success(`Copied ${label}`),
    (e) => notify.error(`Could not copy ${label}`, e)
  );
}

function FileDownload({ entry }: { entry: Entry }) {
  const [pending, setPending] = useState(false);
  if (entry.payload.type !== 'file') return null;
  const data = entry.payload.data;
  return (
    <Button
      size="sm"
      variant="outline"
      disabled={pending || !entry.redactedFields.includes('content')}
      onClick={async () => {
        setPending(true);
        try {
          const { value } = await call('entries.reveal', { id: entry.id, field: 'content' });
          const bytes = Uint8Array.from(atob(value), (c) => c.charCodeAt(0));
          const url = URL.createObjectURL(new Blob([bytes], { type: data.mime }));
          const a = document.createElement('a');
          a.href = url;
          a.download = data.filename;
          a.click();
          setTimeout(() => URL.revokeObjectURL(url), 1000);
        } catch (error) {
          notify.error('Could not export file', error);
        } finally {
          setPending(false);
        }
      }}
    >
      {pending ? <Spinner /> : <DownloadIcon />} Save file…
    </Button>
  );
}

/** Read-only, redacted view of an entry. Secrets are revealed one field at a time. */
export function EntryView({
  entry,
  hideAfterSecs,
  profileName
}: {
  entry: Entry;
  hideAfterSecs: number;
  profileName: string;
}) {
  const copy = useCopyField();
  const has = (path: string) => entry.redactedFields.includes(path);
  const secret = (field: string, label: string, multiline = false) => (
    <SecretField
      empty={!has(field)}
      entryId={entry.id}
      field={field}
      hideAfterSecs={hideAfterSecs}
      label={label}
      multiline={multiline}
    />
  );
  const p = entry.payload;

  switch (p.type) {
    case 'login':
      return <LoginView p={p} entry={entry} hideAfterSecs={hideAfterSecs} secret={secret} />;
    case 'secret':
      return secret('value', 'Secret value');
    case 'env':
      return (
        <EnvEditor
          editable={false}
          entryId={entry.id}
          entryName={entry.name}
          hideAfterSecs={hideAfterSecs}
          profileId={entry.profileId}
          profileName={profileName}
          vars={p.data.vars}
        />
      );
    case 'file':
      return (
        <div className="flex flex-col gap-4">
          <Grid>
            <PlainField label="Filename" mono value={p.data.filename} />
            <PlainField label="Type" mono value={p.data.mime} />
            <PlainField label="Size" value={formatBytes(p.data.size)} />
          </Grid>
          <div>
            <FileDownload entry={entry} />
          </div>
        </div>
      );
    case 'note':
      return secret('body', 'Note', true);
    case 'ssh_key':
      return (
        <div className="flex flex-col gap-4">
          <PlainField
            label="Public key"
            mono
            value={p.data.publicKey}
            onCopy={() => copyPublic('public key', p.data.publicKey)}
          />
          {secret('privateKey', 'Private key', true)}
          <Grid>
            {secret('passphrase', 'Key passphrase')}
            <PlainField label="Comment" value={p.data.comment} />
          </Grid>
        </div>
      );
    case 'database':
      return (
        <div className="flex flex-col gap-4">
          <Grid>
            <PlainField label="Engine" value={p.data.engine} />
            <PlainField
              label="Host"
              mono
              value={`${p.data.host}${p.data.port ? `:${p.data.port}` : ''}`}
            />
            <PlainField label="Database" mono value={p.data.database} />
            <PlainField
              label="Username"
              value={p.data.username}
              onCopy={() => copy.mutate({ id: entry.id, field: 'username', label: 'username' })}
            />
            {secret('password', 'Password')}
            <PlainField label="Options" mono value={p.data.options} />
          </Grid>
          <SecretField
            entryId={entry.id}
            field="url"
            hideAfterSecs={hideAfterSecs}
            label="Connection URL"
          />
        </div>
      );
    case 'api_key':
      return <ApiKeyView p={p} secret={secret} />;
  }
}

function LoginView({
  p,
  entry,
  hideAfterSecs,
  secret
}: {
  p: Extract<EntryPayload, { type: 'login' }>;
  entry: Entry;
  hideAfterSecs: number;
  secret: (field: string, label: string, multiline?: boolean) => React.ReactNode;
}) {
  const copy = useCopyField();
  return (
    <div className="flex flex-col gap-4">
      <Grid>
        <PlainField
          label="Username"
          value={p.data.username}
          onCopy={() => copy.mutate({ id: entry.id, field: 'username', label: 'username' })}
        />
        {secret('password', 'Password')}
        {p.data.totp ? (
          <TotpField entryId={entry.id} hideAfterSecs={hideAfterSecs} period={p.data.totp.period} />
        ) : null}
      </Grid>
      {p.data.urls.length > 0 ? (
        <PlainField label="Websites">
          <ul className="flex flex-col gap-1">
            {p.data.urls.map((url) => (
              <li key={url} className="flex items-center gap-1.5 text-sm">
                <a
                  className="truncate text-primary underline-offset-2 hover:underline"
                  href={url}
                  rel="noreferrer noopener"
                  target="_blank"
                >
                  {url}
                </a>
                <ExternalLinkIcon aria-hidden className="size-3 text-muted-foreground" />
              </li>
            ))}
          </ul>
        </PlainField>
      ) : null}
      {p.data.custom.length > 0 ? (
        <Grid>
          {p.data.custom.map((f) =>
            f.hidden ? (
              <div key={f.key}>{secret(`custom.${f.key}`, f.key)}</div>
            ) : (
              <PlainField key={f.key} label={f.key} value={f.value} />
            )
          )}
        </Grid>
      ) : null}
    </div>
  );
}

function ApiKeyView({
  p,
  secret
}: {
  p: Extract<EntryPayload, { type: 'api_key' }>;
  secret: (field: string, label: string, multiline?: boolean) => React.ReactNode;
}) {
  // This read-only badge intentionally compares the clock at render, without scheduling effects.
  // oxlint-disable-next-line react/purity
  const expired = p.data.expiresAt ? Date.parse(p.data.expiresAt) < Date.now() : false;
  return (
    <Grid>
      {secret('key', 'Key')}
      {secret('secret', 'Secret')}
      <PlainField label="Endpoint" mono value={p.data.endpoint} />
      <PlainField label="Expires">
        <div className="flex min-h-8 items-center gap-2 text-sm">
          {p.data.expiresAt ? (
            new Date(p.data.expiresAt).toLocaleDateString()
          ) : (
            <span className="text-muted-foreground">Never</span>
          )}
          {expired ? <Badge variant="error">Expired</Badge> : null}
        </div>
      </PlainField>
    </Grid>
  );
}
