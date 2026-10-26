import { Button } from '@latch/ui/button';
import { Checkbox } from '@latch/ui/checkbox';
import { Input } from '@latch/ui/input';
import { Switch } from '@latch/ui/switch';
import { Textarea } from '@latch/ui/textarea';
import { FileUpIcon, PlusIcon, ShieldPlusIcon, Trash2Icon, XIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { FormField, PasswordInput } from '~/components/fields';
import { SimpleSelect } from '~/components/simple-select';
import { EnvEditor } from '~/features/env/env-editor';
import { useKeyedRows } from '~/hooks/use-keyed-rows';
import { useAsyncAction } from '~/lib/async-ui';
import { formatBytes, type PayloadOf } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { parseOtpauth } from '~/lib/otpauth';
import type { FieldErrors } from '~/lib/validation';
import type { EntryInput, EntryPayload, TotpConfig } from '../../_generated/contracts';
import domainCatalog from '../../_generated/contracts/domain-catalog.json';

type SetPayload<T extends EntryPayload['type']> = (data: PayloadOf<T>) => void;

function Section({
  title,
  children,
  action
}: {
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <fieldset className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <legend className="font-medium text-muted-foreground text-sm">{title}</legend>
        {action}
      </div>
      {children}
    </fieldset>
  );
}

function Grid({ children }: { children: ReactNode }) {
  return (
    <div className="grid grid-cols-1 gap-4 @min-[28rem]/page-content:grid-cols-2">{children}</div>
  );
}

/** TOTP editor with `otpauth://` paste support (into any of its inputs). */
export function TotpEditor({
  totp,
  onChange,
  errors
}: {
  totp: TotpConfig | null;
  /** `account` is set when an otpauth:// URI was pasted. */
  onChange: (totp: TotpConfig | null, account?: string | null) => void;
  errors: FieldErrors;
}) {
  const parsing = useAsyncAction(totp);
  const applyUri = (text: string) => {
    void parsing.run(
      () => parseOtpauth(text),
      (parsed) => {
        if (parsed) onChange(parsed.totp, parsed.account);
      },
      (error) => notify.error('Could not parse TOTP URI', error)
    );
  };
  const onPaste = (e: React.ClipboardEvent<HTMLElement>) => {
    const text = e.clipboardData.getData('text');
    if (text.trim().startsWith('otpauth://')) {
      e.preventDefault();
      applyUri(text);
    }
  };

  if (!totp) {
    return (
      <div className="flex flex-col gap-2 rounded-xl border border-dashed p-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => onChange(structuredClone(domainCatalog.defaultTotp) as TotpConfig)}
          >
            <ShieldPlusIcon /> Add TOTP
          </Button>
          <span className="text-muted-foreground text-xs">or paste an otpauth:// link:</span>
        </div>
        <Input
          aria-label="Paste otpauth URI"
          aria-busy={parsing.pending}
          className="font-mono"
          placeholder="otpauth://totp/Issuer:account?secret=…"
          size="sm"
          onPaste={onPaste}
          onChange={(e) => {
            applyUri(e.currentTarget.value);
          }}
        />
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-4 rounded-xl border p-3" onPaste={onPaste}>
      <div className="flex items-center justify-between">
        <span className="font-medium text-sm">Two-factor (TOTP)</span>
        <Button
          aria-label="Remove TOTP"
          size="icon-xs"
          variant="ghost-destructive"
          onClick={() => onChange(null)}
        >
          <XIcon />
        </Button>
      </div>
      <FormField
        description="Base32 secret, or paste an otpauth:// link."
        error={errors['totp.secret']}
        label="TOTP secret"
      >
        {(a) => (
          <PasswordInput
            {...a}
            value={totp.secret ?? ''}
            onChange={(secret) => onChange({ ...totp, secret })}
          />
        )}
      </FormField>
      <div className="grid grid-cols-2 gap-4 @min-[36rem]/page-content:grid-cols-4">
        <FormField label="Algorithm">
          {(a) => (
            <SimpleSelect
              id={a.id}
              options={[
                { value: 'SHA1', label: 'SHA-1' },
                { value: 'SHA256', label: 'SHA-256' },
                { value: 'SHA512', label: 'SHA-512' }
              ]}
              value={totp.algorithm}
              onValueChange={(algorithm) => onChange({ ...totp, algorithm })}
            />
          )}
        </FormField>
        <FormField error={errors['totp.digits']} label="Digits">
          {(a) => (
            <SimpleSelect
              id={a.id}
              options={[
                { value: '6', label: '6' },
                { value: '8', label: '8' }
              ]}
              value={String(totp.digits) as '6' | '8'}
              onValueChange={(d) => onChange({ ...totp, digits: Number(d) })}
            />
          )}
        </FormField>
        <FormField error={errors['totp.period']} label="Period (s)">
          {(a) => (
            <Input
              {...a}
              min={10}
              type="number"
              value={totp.period}
              onChange={(e) => onChange({ ...totp, period: Number(e.currentTarget.value) })}
            />
          )}
        </FormField>
        <FormField label="Issuer">
          {(a) => (
            <Input
              {...a}
              value={totp.issuer ?? ''}
              onChange={(e) => onChange({ ...totp, issuer: e.currentTarget.value || null })}
            />
          )}
        </FormField>
      </div>
    </div>
  );
}

function LoginFields({
  data,
  set,
  errors
}: {
  data: PayloadOf<'login'>;
  set: SetPayload<'login'>;
  errors: FieldErrors;
}) {
  const urls = useKeyedRows(data.urls, (urls) => set({ ...data, urls }));
  const custom = useKeyedRows(data.custom, (custom) => set({ ...data, custom }));
  return (
    <div className="flex flex-col gap-5">
      <Grid>
        <FormField label="Username">
          {(a) => (
            <Input
              {...a}
              autoComplete="off"
              value={data.username}
              onChange={(e) => set({ ...data, username: e.currentTarget.value })}
            />
          )}
        </FormField>
        <FormField label="Password">
          {(a) => (
            <PasswordInput
              {...a}
              generate
              value={data.password ?? ''}
              onChange={(password) => set({ ...data, password })}
            />
          )}
        </FormField>
      </Grid>
      <Section
        title="Websites"
        action={
          <Button size="xs" variant="ghost" onClick={() => urls.append('')}>
            <PlusIcon /> Add URL
          </Button>
        }
      >
        {data.urls.length === 0 ? (
          <p className="text-muted-foreground text-xs">No websites yet.</p>
        ) : null}
        {urls.rows.map(({ id, value: url }, i) => (
          <div className="flex items-start gap-2" key={id}>
            <FormField
              className="flex-1"
              error={errors[`urls.${i}`]}
              label={<span className="sr-only">URL {i + 1}</span>}
            >
              {(a) => (
                <Input
                  {...a}
                  placeholder="https://example.com"
                  type="url"
                  value={url}
                  onChange={(e) => urls.update(i, e.currentTarget.value)}
                />
              )}
            </FormField>
            <Button
              aria-label={`Remove URL ${i + 1}`}
              className="mt-5"
              size="icon-sm"
              variant="ghost-destructive"
              onClick={() => urls.remove(i)}
            >
              <Trash2Icon />
            </Button>
          </div>
        ))}
      </Section>
      <Section title="Two-factor">
        <TotpEditor
          errors={errors}
          totp={data.totp}
          onChange={(totp, account) =>
            set({ ...data, totp, username: !data.username && account ? account : data.username })
          }
        />
      </Section>
      <Section
        title="Custom fields"
        action={
          <Button
            size="xs"
            variant="ghost"
            onClick={() => custom.append({ key: '', value: '', hidden: false })}
          >
            <PlusIcon /> Add field
          </Button>
        }
      >
        {custom.rows.map(({ id, value: f }, i) => (
          <div className="grid grid-cols-[1fr_1.4fr_auto_auto] items-start gap-2" key={id}>
            <FormField
              error={errors[`custom.${i}`]}
              label={<span className="sr-only">Field {i + 1} name</span>}
            >
              {(a) => (
                <Input
                  {...a}
                  placeholder="Name"
                  value={f.key}
                  onChange={(e) => custom.update(i, { ...f, key: e.currentTarget.value })}
                />
              )}
            </FormField>
            <FormField label={<span className="sr-only">Field {i + 1} value</span>}>
              {(a) =>
                f.hidden ? (
                  <PasswordInput
                    {...a}
                    value={f.value ?? ''}
                    onChange={(value) => custom.update(i, { ...f, value })}
                  />
                ) : (
                  <Input
                    {...a}
                    placeholder="Value"
                    value={f.value ?? ''}
                    onChange={(e) => custom.update(i, { ...f, value: e.currentTarget.value })}
                  />
                )
              }
            </FormField>
            <label className="mt-6 flex items-center gap-1.5 text-xs">
              <Checkbox
                checked={f.hidden}
                onCheckedChange={(checked) => custom.update(i, { ...f, hidden: checked === true })}
              />
              Hidden
            </label>
            <Button
              aria-label={`Remove field ${f.key || i + 1}`}
              className="mt-5"
              size="icon-sm"
              variant="ghost-destructive"
              onClick={() => custom.remove(i)}
            >
              <Trash2Icon />
            </Button>
          </div>
        ))}
      </Section>
    </div>
  );
}

function LoginPayloadFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'login' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  return (
    <LoginFields
      data={payload.data}
      errors={errors}
      set={(data) => onChange({ type: 'login', data })}
    />
  );
}

function SecretFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'secret' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  const set = (data: PayloadOf<'secret'>) => onChange({ type: 'secret', data });
  return (
    <FormField error={errors.value} label="Secret value">
      {(a) => (
        <PasswordInput
          {...a}
          generate
          value={payload.data.value ?? ''}
          onChange={(value) => set({ value })}
        />
      )}
    </FormField>
  );
}

function EnvFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'env' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  return (
    <EnvEditor
      editable
      errors={errors}
      vars={payload.data.vars}
      onChange={(vars) => onChange({ type: 'env', data: { vars } })}
    />
  );
}

function FileFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'file' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  const d = payload.data;
  return (
    <div className="flex flex-col gap-4">
      <FormField
        error={errors.content}
        label="File"
        description="Stored encrypted; limited to 1 MiB."
      >
        {(a) => (
          <label className="flex cursor-pointer items-center gap-3 rounded-xl border border-dashed p-4 text-sm hover:bg-accent/40">
            <FileUpIcon aria-hidden className="size-5 text-muted-foreground" />
            <span className="flex-1">
              {d.filename ? (
                <>
                  <span className="font-medium">{d.filename}</span>{' '}
                  <span className="text-muted-foreground">({formatBytes(d.size)})</span>
                </>
              ) : (
                'Choose a file…'
              )}
            </span>
            <input
              {...a}
              className="sr-only"
              type="file"
              onChange={async (e) => {
                const file = e.currentTarget.files?.[0];
                if (!file) return;
                const buf = new Uint8Array(await file.arrayBuffer());
                let bin = '';
                for (const b of buf) bin += String.fromCharCode(b);
                onChange({
                  type: 'file',
                  data: {
                    filename: file.name,
                    mime: file.type || 'application/octet-stream',
                    size: file.size,
                    contentBase64: btoa(bin)
                  }
                });
              }}
            />
          </label>
        )}
      </FormField>
      <FormField error={errors.filename} label="Filename">
        {(a) => (
          <Input
            {...a}
            value={d.filename}
            onChange={(e) =>
              onChange({ type: 'file', data: { ...d, filename: e.currentTarget.value } })
            }
          />
        )}
      </FormField>
    </div>
  );
}

function NoteFields({
  payload,
  onChange
}: {
  payload: Extract<EntryPayload, { type: 'note' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  return (
    <FormField label="Note (Markdown)">
      {(a) => (
        <Textarea
          {...a}
          rows={10}
          value={payload.data.body ?? ''}
          onChange={(e) => onChange({ type: 'note', data: { body: e.currentTarget.value } })}
        />
      )}
    </FormField>
  );
}

function SshKeyFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'ssh_key' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  const d = payload.data;
  const set = (patch: Partial<PayloadOf<'ssh_key'>>) =>
    onChange({ type: 'ssh_key', data: { ...d, ...patch } });
  return (
    <div className="flex flex-col gap-4">
      <FormField error={errors.privateKey} label="Private key">
        {(a) => (
          <Textarea
            {...a}
            className="font-mono text-xs"
            placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
            rows={6}
            spellCheck={false}
            value={d.privateKey ?? ''}
            onChange={(e) => set({ privateKey: e.currentTarget.value })}
          />
        )}
      </FormField>
      <FormField error={errors.publicKey} label="Public key">
        {(a) => (
          <Input
            {...a}
            className="font-mono"
            placeholder="ssh-ed25519 AAAA…"
            value={d.publicKey}
            onChange={(e) => set({ publicKey: e.currentTarget.value })}
          />
        )}
      </FormField>
      <Grid>
        <FormField label="Key passphrase">
          {(a) => (
            <PasswordInput
              {...a}
              placeholder={d.passphrase === null ? 'None' : undefined}
              value={d.passphrase ?? ''}
              // Deliberate clear sends "" (protocol: clears the stored value);
              // null is reserved for untouched/redacted fields (keep).
              onChange={(passphrase) => set({ passphrase })}
            />
          )}
        </FormField>
        <FormField label="Comment">
          {(a) => (
            <Input
              {...a}
              value={d.comment}
              onChange={(e) => set({ comment: e.currentTarget.value })}
            />
          )}
        </FormField>
      </Grid>
    </div>
  );
}

function DatabaseFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'database' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  const d = payload.data;
  const set = (patch: Partial<PayloadOf<'database'>>) =>
    onChange({ type: 'database', data: { ...d, ...patch } });
  const ports: Record<string, number | null> = {
    postgres: 5432,
    mysql: 3306,
    mongodb: 27017,
    redis: 6379,
    sqlite: null,
    other: null
  };
  return (
    <Grid>
      <FormField error={errors.engine} label="Engine">
        {(a) => (
          <SimpleSelect
            id={a.id}
            options={['postgres', 'mysql', 'mongodb', 'redis', 'sqlite', 'other'].map((v) => ({
              value: v,
              label: v
            }))}
            value={d.engine}
            onValueChange={(engine) =>
              set({
                engine,
                port: d.port === ports[d.engine] ? (ports[engine] ?? null) : d.port
              })
            }
          />
        )}
      </FormField>
      <FormField error={errors.host} label="Host">
        {(a) => (
          <Input {...a} value={d.host} onChange={(e) => set({ host: e.currentTarget.value })} />
        )}
      </FormField>
      <FormField error={errors.port} label="Port">
        {(a) => (
          <Input
            {...a}
            type="number"
            value={d.port ?? ''}
            onChange={(e) =>
              set({ port: e.currentTarget.value === '' ? null : Number(e.currentTarget.value) })
            }
          />
        )}
      </FormField>
      <FormField label="Database">
        {(a) => (
          <Input
            {...a}
            value={d.database}
            onChange={(e) => set({ database: e.currentTarget.value })}
          />
        )}
      </FormField>
      <FormField label="Username">
        {(a) => (
          <Input
            {...a}
            autoComplete="off"
            value={d.username}
            onChange={(e) => set({ username: e.currentTarget.value })}
          />
        )}
      </FormField>
      <FormField label="Password">
        {(a) => (
          <PasswordInput
            {...a}
            generate
            value={d.password ?? ''}
            onChange={(password) => set({ password })}
          />
        )}
      </FormField>
      <FormField
        className="col-span-full"
        description="Query-string style, e.g. sslmode=require"
        label="Options"
      >
        {(a) => (
          <Input
            {...a}
            className="font-mono"
            value={d.options}
            onChange={(e) => set({ options: e.currentTarget.value })}
          />
        )}
      </FormField>
    </Grid>
  );
}

function ApiKeyFields({
  payload,
  onChange,
  errors
}: {
  payload: Extract<EntryPayload, { type: 'api_key' }>;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  const d = payload.data;
  const set = (patch: Partial<PayloadOf<'api_key'>>) =>
    onChange({ type: 'api_key', data: { ...d, ...patch } });
  return (
    <Grid>
      <FormField error={errors.key} label="Key">
        {(a) => <PasswordInput {...a} value={d.key ?? ''} onChange={(key) => set({ key })} />}
      </FormField>
      <FormField label="Secret (optional)">
        {(a) => (
          <PasswordInput
            {...a}
            placeholder={d.secret === null ? 'None' : undefined}
            value={d.secret ?? ''}
            // "" clears the stored secret on update; null keeps it.
            onChange={(secret) => set({ secret })}
          />
        )}
      </FormField>
      <FormField error={errors.endpoint} label="Endpoint">
        {(a) => (
          <Input
            {...a}
            placeholder="https://api.example.com"
            value={d.endpoint ?? ''}
            onChange={(e) => set({ endpoint: e.currentTarget.value || null })}
          />
        )}
      </FormField>
      <FormField error={errors.expiresAt} label="Expires">
        {(a) => (
          <Input
            {...a}
            type="date"
            value={d.expiresAt ? d.expiresAt.slice(0, 10) : ''}
            onChange={(e) =>
              set({
                expiresAt: e.currentTarget.value ? `${e.currentTarget.value}T00:00:00Z` : null
              })
            }
          />
        )}
      </FormField>
    </Grid>
  );
}

function PayloadFields({
  payload,
  onChange,
  errors
}: {
  payload: EntryPayload;
  onChange: (payload: EntryPayload) => void;
  errors: FieldErrors;
}) {
  switch (payload.type) {
    case 'login':
      return <LoginPayloadFields payload={payload} onChange={onChange} errors={errors} />;
    case 'secret':
      return <SecretFields payload={payload} onChange={onChange} errors={errors} />;
    case 'env':
      return <EnvFields payload={payload} onChange={onChange} errors={errors} />;
    case 'file':
      return <FileFields payload={payload} onChange={onChange} errors={errors} />;
    case 'note':
      return <NoteFields payload={payload} onChange={onChange} errors={errors} />;
    case 'ssh_key':
      return <SshKeyFields payload={payload} onChange={onChange} errors={errors} />;
    case 'database':
      return <DatabaseFields payload={payload} onChange={onChange} errors={errors} />;
    case 'api_key':
      return <ApiKeyFields payload={payload} onChange={onChange} errors={errors} />;
  }
}

function TagsInput({
  tags,
  onChange,
  ...props
}: Omit<React.ComponentProps<typeof Input>, 'value' | 'onChange'> & {
  tags: string[];
  onChange: (tags: string[]) => void;
}) {
  const [text, setText] = useState(() => tags.join(', '));
  return (
    <Input
      {...props}
      value={text}
      onChange={(event) => {
        const next = event.currentTarget.value;
        setText(next);
        onChange(
          next
            .split(',')
            .map((tag) => tag.trim())
            .filter(Boolean)
        );
      }}
    />
  );
}

export function EntryForm({
  value,
  onChange,
  errors
}: {
  value: EntryInput;
  onChange: (value: EntryInput) => void;
  errors: FieldErrors;
}) {
  return (
    <div className="flex flex-col gap-6" data-testid="entry-form">
      <Grid>
        <FormField error={errors.name} label="Name">
          {(a) => (
            <Input
              {...a}
              autoFocus
              value={value.name}
              onChange={(e) => onChange({ ...value, name: e.currentTarget.value })}
            />
          )}
        </FormField>
        <FormField description="Comma separated" label="Tags">
          {(a) => (
            <TagsInput {...a} tags={value.tags} onChange={(tags) => onChange({ ...value, tags })} />
          )}
        </FormField>
      </Grid>
      <PayloadFields
        errors={errors}
        payload={value.payload}
        onChange={(payload) => onChange({ ...value, payload })}
      />
      <FormField label="Notes">
        {(a) => (
          <Textarea
            {...a}
            rows={3}
            value={value.notes}
            onChange={(e) => onChange({ ...value, notes: e.currentTarget.value })}
          />
        )}
      </FormField>
      <label className="flex items-center gap-2 text-sm">
        <Switch
          checked={value.favorite}
          onCheckedChange={(favorite) => onChange({ ...value, favorite })}
        />
        Favorite (pinned in the tray when it has TOTP)
      </label>
    </div>
  );
}
