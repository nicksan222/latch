import { create } from '@bufbuild/protobuf';
import { EnvVarSchema } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import { EnvFormat, type EnvVar } from '@latch/rpc/v1';
import { Button } from '@latch/ui/button';
import { Checkbox } from '@latch/ui/checkbox';
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle
} from '@latch/ui/dialog';
import { useCopyToClipboard } from '@latch/ui/hooks/use-copy-to-clipboard';
import { Input } from '@latch/ui/input';
import { Menu, MenuItem, MenuPopup, MenuTrigger } from '@latch/ui/menu';
import { Spinner } from '@latch/ui/spinner';
import { Switch } from '@latch/ui/switch';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@latch/ui/table';
import { Textarea } from '@latch/ui/textarea';
import { cn } from '@latch/ui/utils';
import {
  ArrowDownIcon,
  ArrowUpIcon,
  ClipboardCopyIcon,
  FileUpIcon,
  PlayIcon,
  PlusIcon,
  TerminalIcon,
  Trash2Icon
} from 'lucide-react';
import { useId, useState } from 'react';
import { SecretField } from '~/components/secret-field';
import { useKeyedRows } from '~/hooks/use-keyed-rows';
import { call } from '~/lib/api/client';
import { notify } from '~/lib/notify';

/** Merge parsed vars into the table: same key replaces in place, new keys append. */
export function mergeEnvVars(current: readonly EnvVar[], incoming: readonly EnvVar[]): EnvVar[] {
  const next = [...current];
  for (const v of incoming) {
    const i = next.findIndex((c) => c.key === v.key);
    if (i >= 0) next[i] = { ...v, comment: v.comment ?? next[i]!.comment };
    else next.push(v);
  }
  return next;
}

export function moveItem<T>(list: readonly T[], from: number, to: number): T[] {
  if (to < 0 || to >= list.length) return [...list];
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item!);
  return next;
}

function shellWord(s: string): string {
  return /^[A-Za-z0-9_./:@+-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`;
}

/** The CLI equivalent shown by the Run… dialog (SPEC §8: `latch env <profile> [entry ...] -- <cmd>`). */
export function runCommand(profileName: string, entryName: string | null, command: string): string {
  const entry = entryName ? ` ${shellWord(entryName)}` : '';
  return `latch env ${shellWord(profileName)}${entry} -- ${command.trim() || '<command>'}`;
}

function ImportEnvDialog({
  open,
  onOpenChange,
  onImport
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImport: (vars: EnvVar[]) => void;
}) {
  const [text, setText] = useState('');
  const [pending, setPending] = useState(false);
  const id = useId();
  const parse = async () => {
    setPending(true);
    try {
      const { items: vars } = await call(LatchAgent.method.envParse, { text });
      if (vars.length === 0) {
        notify.warning('No variables found', 'Expected KEY=value lines.');
        return;
      }
      onImport(vars);
      notify.success(`Imported ${vars.length} variable${vars.length === 1 ? '' : 's'}`);
      setText('');
      onOpenChange(false);
    } catch (error) {
      notify.error('Could not parse .env', error);
    } finally {
      setPending(false);
    }
  };
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>Import .env</DialogTitle>
          <DialogDescription>
            Paste the contents of a .env file or choose one. Values that look like credentials are
            marked hidden.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="flex flex-col gap-3">
          <label className="font-medium text-xs" htmlFor={id}>
            .env contents
          </label>
          <Textarea
            className="font-mono text-xs"
            id={id}
            placeholder={'API_URL=https://api.example.com\nAPI_TOKEN=…'}
            rows={8}
            value={text}
            onChange={(e) => setText(e.currentTarget.value)}
          />
          <label className="inline-flex cursor-pointer items-center gap-2 text-muted-foreground text-xs">
            <FileUpIcon className="size-3.5" aria-hidden />
            <span className="underline underline-offset-2">Choose a file…</span>
            <input
              accept=".env,text/plain,*/*"
              aria-label="Choose a .env file"
              className="sr-only"
              type="file"
              onChange={async (e) => {
                const file = e.currentTarget.files?.[0];
                if (file) setText(await file.text());
              }}
            />
          </label>
        </DialogPanel>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button disabled={!text.trim() || pending} onClick={() => void parse()}>
            {pending ? <Spinner /> : null}
            Import
          </Button>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

function RunDialog({
  open,
  onOpenChange,
  profileName,
  entryName
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  profileName: string;
  entryName: string;
}) {
  const [command, setCommand] = useState('bun run dev');
  const [allEntries, setAllEntries] = useState(false);
  const { copyToClipboard, isCopied } = useCopyToClipboard({ target: 'command' });
  const cli = runCommand(profileName, allEntries ? null : entryName, command);
  const id = useId();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>Run with these variables</DialogTitle>
          <DialogDescription>
            Latch injects the variables into the process environment. Nothing is written to disk.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label className="font-medium text-xs" htmlFor={id}>
              Command
            </label>
            <Input
              className="font-mono"
              id={id}
              value={command}
              onChange={(e) => setCommand(e.currentTarget.value)}
            />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <Switch checked={allEntries} onCheckedChange={setAllEntries} />
            Merge all env entries of “{profileName}”
          </label>
          <div className="rounded-lg border bg-muted/50 p-3">
            <div className="mb-1.5 flex items-center gap-1.5 text-muted-foreground text-xs">
              <TerminalIcon className="size-3.5" aria-hidden /> Run in your terminal
            </div>
            <code className="block break-all font-mono text-xs" data-testid="run-command">
              {cli}
            </code>
          </div>
        </DialogPanel>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          <Button onClick={() => copyToClipboard(cli, undefined)}>
            <ClipboardCopyIcon />
            {isCopied ? 'Copied' : 'Copy command'}
          </Button>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

export interface EnvEditorProps {
  vars: EnvVar[];
  /** Editable table (edit/create mode). */
  editable: boolean;
  onChange?: (vars: EnvVar[]) => void;
  /** Saved entry: enables Copy as .env / exports, Run…, per-var reveal. */
  entryId?: string;
  entryName?: string;
  profileId?: string;
  profileName?: string;
  hideAfterSecs?: number;
  errors?: import('~/lib/validation').FieldErrors;
}

/** SPEC §9.2.5 env editor: key / value / hidden / comment table. */
export function EnvEditor({
  vars,
  editable,
  onChange,
  entryId,
  entryName,
  profileId,
  profileName,
  hideAfterSecs = 20,
  errors = {}
}: EnvEditorProps) {
  const [importOpen, setImportOpen] = useState(false);
  const [runOpen, setRunOpen] = useState(false);
  const [busy, setBusy] = useState<EnvFormat | null>(null);
  const keyed = useKeyedRows(vars, (next) => onChange?.(next));
  const update = (i: number, patch: Partial<EnvVar>) => keyed.update(i, { ...vars[i]!, ...patch });

  const copyAs = async (format: EnvFormat) => {
    if (!entryId || !profileId) return;
    setBusy(format);
    try {
      // The agent formats and writes the clipboard itself (auto-clear applies):
      // the plaintext env never enters the webview (AGENTS.md rule 7).
      const { clearAfterSecs } = await call(LatchAgent.method.envCopy, {
        profileId,
        entryIds: { items: [entryId] },
        format
      });
      notify.success(
        format === EnvFormat.EXPORT
          ? 'Copied as exports'
          : format === EnvFormat.JSON
            ? 'Copied as JSON'
            : 'Copied as .env',
        clearAfterSecs > 0 ? `Clipboard clears in ${clearAfterSecs}s` : undefined
      );
    } catch (error) {
      notify.error('Could not copy', error);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-col gap-3" data-testid="env-editor">
      <div className="flex flex-wrap items-center gap-2">
        {editable ? (
          <>
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                keyed.append(create(EnvVarSchema, { key: '', value: '', hidden: true }))
              }
            >
              <PlusIcon /> Add variable
            </Button>
            <Button size="sm" variant="outline" onClick={() => setImportOpen(true)}>
              <FileUpIcon /> Import .env
            </Button>
          </>
        ) : null}
        {!editable && entryId ? (
          <>
            <Menu>
              <MenuTrigger render={<Button size="sm" variant="outline" disabled={busy !== null} />}>
                {busy ? <Spinner /> : <ClipboardCopyIcon />} Copy as…
              </MenuTrigger>
              <MenuPopup align="start">
                <MenuItem onClick={() => void copyAs(EnvFormat.DOTENV)}>Copy as .env</MenuItem>
                <MenuItem onClick={() => void copyAs(EnvFormat.EXPORT)}>Copy as exports</MenuItem>
                <MenuItem onClick={() => void copyAs(EnvFormat.JSON)}>Copy as JSON</MenuItem>
              </MenuPopup>
            </Menu>
            <Button size="sm" variant="outline" onClick={() => setRunOpen(true)}>
              <PlayIcon /> Run…
            </Button>
          </>
        ) : null}
        <span className="ms-auto text-muted-foreground text-xs">
          {vars.length} variable{vars.length === 1 ? '' : 's'}
        </span>
      </div>

      <div className="overflow-x-auto rounded-xl border border-border/60">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[30%]">Key</TableHead>
              <TableHead>Value</TableHead>
              <TableHead className="w-16 text-center">Hidden</TableHead>
              <TableHead className="w-[22%]">Comment</TableHead>
              {editable ? (
                <TableHead className="w-24">
                  <span className="sr-only">Actions</span>
                </TableHead>
              ) : null}
            </TableRow>
          </TableHeader>
          <TableBody>
            {vars.length === 0 ? (
              <TableRow>
                <TableCell
                  className="py-6 text-center text-muted-foreground"
                  colSpan={editable ? 5 : 4}
                >
                  No variables yet.
                </TableCell>
              </TableRow>
            ) : null}
            {keyed.rows.map(({ id, value: v }, i) => (
              <TableRow key={editable ? id : v.key} data-testid="env-row">
                <TableCell className="align-top">
                  {editable ? (
                    <div className="flex flex-col gap-1">
                      <Input
                        aria-invalid={errors[`vars.${i}`] ? true : undefined}
                        aria-label={`Variable ${i + 1} key`}
                        className="font-mono"
                        placeholder="KEY"
                        size="sm"
                        value={v.key}
                        onChange={(e) =>
                          update(i, {
                            key: e.currentTarget.value.toUpperCase().replace(/\s/g, '_')
                          })
                        }
                      />
                      {errors[`vars.${i}`] ? (
                        <span className="text-destructive-foreground text-xs" role="alert">
                          {errors[`vars.${i}`]}
                        </span>
                      ) : null}
                    </div>
                  ) : (
                    <span className="font-mono text-xs">{v.key}</span>
                  )}
                </TableCell>
                <TableCell className="align-top">
                  {editable ? (
                    <Input
                      aria-label={`${v.key || `Variable ${i + 1}`} value`}
                      className={cn(
                        'font-mono',
                        v.hidden && '[&_input:not(:focus)]:[-webkit-text-security:disc]'
                      )}
                      placeholder="value"
                      size="sm"
                      value={v.value ?? ''}
                      onChange={(e) => update(i, { value: e.currentTarget.value })}
                    />
                  ) : v.hidden && entryId ? (
                    <SecretField
                      compact
                      entryId={entryId}
                      field={`vars.${v.key}`}
                      hideAfterSecs={hideAfterSecs}
                      label={v.key}
                    />
                  ) : (
                    <span className="break-all font-mono text-xs">{v.value}</span>
                  )}
                </TableCell>
                <TableCell className="text-center align-top">
                  {editable ? (
                    <Checkbox
                      aria-label={`Hide ${v.key || `variable ${i + 1}`}`}
                      checked={v.hidden}
                      className="mt-1.5"
                      onCheckedChange={(checked) => update(i, { hidden: checked === true })}
                    />
                  ) : (
                    <span className="text-muted-foreground text-xs">{v.hidden ? 'Yes' : 'No'}</span>
                  )}
                </TableCell>
                <TableCell className="align-top">
                  {editable ? (
                    <Input
                      aria-label={`${v.key || `Variable ${i + 1}`} comment`}
                      placeholder="—"
                      size="sm"
                      value={v.comment ?? ''}
                      onChange={(e) => update(i, { comment: e.currentTarget.value || undefined })}
                    />
                  ) : (
                    <span className="text-muted-foreground text-xs">{v.comment}</span>
                  )}
                </TableCell>
                {editable ? (
                  <TableCell className="align-top">
                    <div className="flex items-center justify-end gap-0.5">
                      <Button
                        aria-label={`Move ${v.key || 'variable'} up`}
                        disabled={i === 0}
                        size="icon-xs"
                        variant="ghost"
                        onClick={() => keyed.move(i, i - 1)}
                      >
                        <ArrowUpIcon />
                      </Button>
                      <Button
                        aria-label={`Move ${v.key || 'variable'} down`}
                        disabled={i === vars.length - 1}
                        size="icon-xs"
                        variant="ghost"
                        onClick={() => keyed.move(i, i + 1)}
                      >
                        <ArrowDownIcon />
                      </Button>
                      <Button
                        aria-label={`Remove ${v.key || 'variable'}`}
                        size="icon-xs"
                        variant="ghost-destructive"
                        onClick={() => keyed.remove(i)}
                      >
                        <Trash2Icon />
                      </Button>
                    </div>
                  </TableCell>
                ) : null}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      <ImportEnvDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        onImport={(incoming) => onChange?.(mergeEnvVars(vars, incoming))}
      />
      {entryId ? (
        <RunDialog
          entryName={entryName ?? ''}
          open={runOpen}
          profileName={profileName ?? ''}
          onOpenChange={setRunOpen}
        />
      ) : null}
    </div>
  );
}
