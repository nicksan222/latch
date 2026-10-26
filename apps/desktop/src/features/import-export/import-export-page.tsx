import { create } from '@bufbuild/protobuf';
import { ExportFormat, ImportFormat, ImportParamsSchema } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { ImportPreview } from '@latch/rpc/v1';
import { Alert, AlertDescription, AlertTitle } from '@latch/ui/alert';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import { Checkbox } from '@latch/ui/checkbox';
import { Input } from '@latch/ui/input';
import { Spinner } from '@latch/ui/spinner';
import { Switch } from '@latch/ui/switch';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@latch/ui/table';
import { Tabs, TabsList, TabsPanel, TabsTab } from '@latch/ui/tabs';
import {
  DownloadIcon,
  EyeIcon,
  FileInputIcon,
  FolderOpenIcon,
  TriangleAlertIcon,
  UploadIcon
} from 'lucide-react';
import { useMemo, useState } from 'react';
import { ConfirmDialog } from '~/components/confirm-dialog';
import { FormField, PasswordInput } from '~/components/fields';
import { EntryTypeIcon } from '~/components/icons';
import { PageContainer, PageTitle, SettingsSection } from '~/components/page';
import { SimpleSelect } from '~/components/simple-select';
import { AppHeader } from '~/features/shell/app-header';
import { call } from '~/lib/api/client';
import { qk, useProfiles, useRpcMutation } from '~/lib/api/queries';
import { isTauri } from '~/lib/api/transport';
import { pickPath } from '~/lib/clipboard';
import { ENTRY_TYPE_LABEL } from '~/lib/entries';
import { notify } from '~/lib/notify';

const IMPORT_FORMATS: { value: ImportFormat; label: string }[] = [
  { value: ImportFormat.DOTENV, label: '.env file' },
  { value: ImportFormat.BITWARDEN, label: 'Bitwarden JSON' },
  { value: ImportFormat.ONE_PASSWORD_CSV, label: '1Password CSV' },
  { value: ImportFormat.KEEPASS_XML, label: 'KeePass XML' },
  { value: ImportFormat.BROWSER_CSV, label: 'Chrome / Firefox passwords CSV' },
  { value: ImportFormat.PASS, label: 'pass store (directory)' },
  { value: ImportFormat.JSON, label: 'Generic JSON' },
  { value: ImportFormat.YAML, label: 'Generic YAML / SOPS' },
  { value: ImportFormat.LATCH_ARCHIVE, label: 'Latch encrypted archive' }
];

const EXPORT_FORMATS: { value: ExportFormat; label: string }[] = [
  { value: ExportFormat.LATCH_ARCHIVE, label: 'Encrypted archive (.latch.age)' },
  { value: ExportFormat.DOTENV, label: '.env (env entries only)' },
  { value: ExportFormat.JSON, label: 'Plaintext JSON' }
];

const ALL = '__all__';

function PathInput({
  value,
  onChange,
  save,
  label
}: {
  value: string;
  onChange: (v: string) => void;
  save?: boolean;
  label: string;
}) {
  return (
    <FormField label={label}>
      {(a) => (
        <div className="flex gap-2">
          <Input
            {...a}
            className="font-mono"
            placeholder={save ? '~/Desktop/latch-export' : '~/Downloads/export.json'}
            value={value}
            onChange={(e) => onChange(e.currentTarget.value)}
          />
          {isTauri() ? (
            <Button
              aria-label="Browse"
              size="icon"
              variant="outline"
              onClick={async () => {
                const p = await pickPath(save ? { save: true, title: label } : { title: label });
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

function ImportPanel() {
  const profiles = useProfiles();
  const [format, setFormat] = useState<ImportFormat>(ImportFormat.BITWARDEN);
  const [path, setPath] = useState('');
  const [profileId, setProfileId] = useState<string>(ALL);
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [passphrase, setPassphrase] = useState('');
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const params = create(ImportParamsSchema, {
    format,
    path: path.trim(),
    profileId: profileId === ALL ? undefined : profileId,
    skipDuplicates,
    passphrase: format === ImportFormat.LATCH_ARCHIVE ? passphrase : undefined
  });
  const apply = useRpcMutation(LatchAgent.method.importApply, {
    invalidate: [qk.entries, qk.profiles, qk.sync],
    onSuccess: (r) => {
      notify.success(
        `Imported ${r.created} entr${r.created === 1 ? 'y' : 'ies'}`,
        `${r.skipped} skipped · ${r.profilesCreated} profile(s) created`
      );
      setPreview(null);
    },
    onError: (e) => notify.error('Import failed', e)
  });
  const runPreview = async () => {
    setPreviewing(true);
    try {
      setPreview(await call(LatchAgent.method.importPreview, params));
    } catch (e) {
      notify.error('Preview failed', e);
      setPreview(null);
    } finally {
      setPreviewing(false);
    }
  };
  const dupes = preview?.items.filter((i) => i.duplicate).length ?? 0;
  return (
    <div className="flex flex-col gap-6">
      <SettingsSection title="Source">
        <div className="grid gap-4 p-4 sm:grid-cols-2">
          <FormField label="Format">
            {(a) => (
              <SimpleSelect
                id={a.id}
                options={IMPORT_FORMATS.map((option) => ({
                  ...option,
                  value: String(option.value)
                }))}
                value={String(format)}
                onValueChange={(f) => {
                  setFormat(Number(f) as ImportFormat);
                  setPreview(null);
                }}
              />
            )}
          </FormField>
          <PathInput
            label="File or directory"
            value={path}
            onChange={(v) => {
              setPath(v);
              setPreview(null);
            }}
          />
          <FormField
            description="“From source” maps folders/collections to profiles."
            label="Target profile"
          >
            {(a) => (
              <SimpleSelect
                id={a.id}
                options={[
                  { value: ALL, label: 'From source (create as needed)' },
                  ...(profiles.data ?? []).map((p) => ({ value: p.id, label: p.name }))
                ]}
                value={profileId}
                onValueChange={(v) => {
                  setProfileId(v);
                  setPreview(null);
                }}
              />
            )}
          </FormField>
          {format === ImportFormat.LATCH_ARCHIVE ? (
            <FormField label="Archive passphrase">
              {(a) => <PasswordInput {...a} value={passphrase} onChange={setPassphrase} />}
            </FormField>
          ) : null}
          <label className="flex items-center gap-2 text-sm sm:col-span-2">
            <Switch checked={skipDuplicates} onCheckedChange={setSkipDuplicates} /> Skip duplicates
            (same name in the same profile)
          </label>
        </div>
      </SettingsSection>
      <div className="flex gap-2">
        <Button
          disabled={!path.trim() || previewing}
          variant="outline"
          onClick={() => void runPreview()}
        >
          {previewing ? <Spinner /> : <EyeIcon />} Preview
        </Button>
        <Button
          disabled={!preview || preview.items.length === 0 || apply.isPending}
          onClick={() => apply.mutate(params)}
        >
          {apply.isPending ? <Spinner /> : <UploadIcon />} Import{' '}
          {preview ? preview.items.length - (skipDuplicates ? dupes : 0) : ''}
        </Button>
      </div>
      {preview ? (
        <ImportPreviewTable preview={preview} dupes={dupes} skipDuplicates={skipDuplicates} />
      ) : null}
    </div>
  );
}

function ImportPreviewTable({
  preview,
  dupes,
  skipDuplicates
}: {
  preview: ImportPreview;
  dupes: number;
  skipDuplicates: boolean;
}) {
  const rows = useMemo(
    () => preview.items.map((value) => ({ id: crypto.randomUUID(), value })),
    [preview.items]
  );
  return (
    <SettingsSection
      headerAction={
        dupes ? (
          <Badge variant="warning">
            {dupes} duplicate{dupes === 1 ? '' : 's'}
          </Badge>
        ) : null
      }
      title="Preview"
    >
      {preview.warnings.map((w) => (
        <Alert variant="warning" key={w}>
          <TriangleAlertIcon aria-hidden />
          <AlertDescription>{w}</AlertDescription>
        </Alert>
      ))}
      <Table data-testid="import-preview">
        <TableHeader>
          <TableRow>
            <TableHead>Name</TableHead>
            <TableHead>Type</TableHead>
            <TableHead>Profile</TableHead>
            <TableHead className="text-right">Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map(({ id, value: i }) => (
            <TableRow key={id}>
              <TableCell className="font-medium">{i.name}</TableCell>
              <TableCell>
                <span className="inline-flex items-center gap-1.5">
                  <EntryTypeIcon className="size-3.5 text-muted-foreground" type={i.type} />{' '}
                  {ENTRY_TYPE_LABEL[i.type]}
                </span>
              </TableCell>
              <TableCell>{i.profileName}</TableCell>
              <TableCell className="text-right">
                {i.duplicate ? (
                  <Badge variant="warning">
                    {skipDuplicates ? 'skip · duplicate' : 'duplicate'}
                  </Badge>
                ) : (
                  <Badge variant="success">new</Badge>
                )}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </SettingsSection>
  );
}

function ExportPanel() {
  const profiles = useProfiles();
  const [format, setFormat] = useState<ExportFormat>(ExportFormat.LATCH_ARCHIVE);
  const [path, setPath] = useState('');
  const [profileId, setProfileId] = useState<string>(ALL);
  const [passphrase, setPassphrase] = useState('');
  const [understood, setUnderstood] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const run = useRpcMutation(LatchAgent.method.exportRun, {
    onSuccess: (r) => {
      notify.success(`Exported ${r.entries} entr${r.entries === 1 ? 'y' : 'ies'}`, r.path);
      setConfirmOpen(false);
      setPassphrase('');
    },
    onError: (e) => notify.error('Export failed', e)
  });
  const plaintext = format !== ExportFormat.LATCH_ARCHIVE;
  const doExport = () =>
    run.mutate({
      format,
      path: path.trim(),
      profileId: profileId === ALL ? undefined : profileId,
      passphrase: format === ExportFormat.LATCH_ARCHIVE ? passphrase : undefined
    });
  const ready = path.trim() && (format !== ExportFormat.LATCH_ARCHIVE || passphrase.length >= 8);
  return (
    <div className="flex flex-col gap-6">
      <SettingsSection title="Export">
        <div className="grid gap-4 p-4 sm:grid-cols-2">
          <FormField label="Format">
            {(a) => (
              <SimpleSelect
                id={a.id}
                options={EXPORT_FORMATS.map((option) => ({
                  ...option,
                  value: String(option.value)
                }))}
                value={String(format)}
                onValueChange={(f) => {
                  setFormat(Number(f) as ExportFormat);
                  setUnderstood(false);
                }}
              />
            )}
          </FormField>
          <FormField label="Profiles">
            {(a) => (
              <SimpleSelect
                id={a.id}
                options={[
                  { value: ALL, label: 'All profiles' },
                  ...(profiles.data ?? []).map((p) => ({ value: p.id, label: p.name }))
                ]}
                value={profileId}
                onValueChange={setProfileId}
              />
            )}
          </FormField>
          <PathInput label="Destination" save value={path} onChange={setPath} />
          {format === ExportFormat.LATCH_ARCHIVE ? (
            <FormField
              description="At least 8 characters. Needed to re-import."
              label="Archive passphrase"
            >
              {(a) => (
                <PasswordInput
                  {...a}
                  autoComplete="new-password"
                  value={passphrase}
                  onChange={setPassphrase}
                />
              )}
            </FormField>
          ) : null}
        </div>
      </SettingsSection>
      {plaintext ? (
        <Alert variant="error" data-testid="plaintext-warning">
          <TriangleAlertIcon />
          <AlertTitle>This writes your secrets to disk in plaintext</AlertTitle>
          <AlertDescription>
            Anyone or any program that can read the file gets every exported secret. Prefer the
            encrypted archive; delete the file securely after use.
            <label className="flex items-center gap-2 text-foreground">
              <Checkbox checked={understood} onCheckedChange={(v) => setUnderstood(v === true)} /> I
              understand the risk
            </label>
          </AlertDescription>
        </Alert>
      ) : null}
      <div>
        <Button
          disabled={!ready || (plaintext && !understood) || run.isPending}
          variant={plaintext ? 'destructive' : 'default'}
          onClick={() => (plaintext ? setConfirmOpen(true) : doExport())}
        >
          {run.isPending ? <Spinner /> : <DownloadIcon />}{' '}
          {plaintext ? 'Export plaintext…' : 'Export'}
        </Button>
      </div>
      <ConfirmDialog
        confirmLabel="Write plaintext file"
        description={`Secrets will be written unencrypted to ${path.trim()}.`}
        open={confirmOpen}
        pending={run.isPending}
        title="Export secrets in plaintext?"
        onConfirm={doExport}
        onOpenChange={setConfirmOpen}
      />
    </div>
  );
}

export function ImportExportPage() {
  return (
    <>
      <AppHeader title={<PageTitle>Import / Export</PageTitle>} />
      <PageContainer>
        <Tabs defaultValue="import">
          <TabsList>
            <TabsTab value="import">
              <FileInputIcon /> Import
            </TabsTab>
            <TabsTab value="export">
              <DownloadIcon /> Export
            </TabsTab>
          </TabsList>
          <TabsPanel className="pt-4" value="import">
            <ImportPanel />
          </TabsPanel>
          <TabsPanel className="pt-4" value="export">
            <ExportPanel />
          </TabsPanel>
        </Tabs>
      </PageContainer>
    </>
  );
}
