import { Resolution } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { ConflictDetail, ConflictSummary, DomainDiffRow } from '@latch/rpc/v1';
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
import { Radio, RadioGroup } from '@latch/ui/radio-group';
import { Spinner } from '@latch/ui/spinner';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@latch/ui/table';
import { useEffect, useRef, useState } from 'react';
import { call } from '~/lib/api/client';
import { qk, useRpcMutation } from '~/lib/api/queries';
import { useAsyncAction } from '~/lib/async-ui';
import { domain } from '~/lib/domain';
import { entryToInput, mergeEntries } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { relativeTime } from '~/lib/time';

const SECRET_KEYS = /^(password|value|body|privateKey|key|secret|totp\.secret|custom\.|vars\.)/;

export function diffRows(detail: ConflictDetail): Promise<DomainDiffRow[]> {
  return domain('diffRows', {
    mine: detail.mine ? entryToInput(detail.mine) : undefined,
    theirs: detail.theirs ? entryToInput(detail.theirs) : undefined
  }).then((response) => response.items);
}

function Value({
  field,
  value,
  contentDiffers
}: {
  field: string;
  value: string;
  contentDiffers?: boolean | undefined;
}) {
  if (!value) return <span className="text-muted-foreground">—</span>;
  return (
    <span className={SECRET_KEYS.test(field) ? 'break-all font-mono' : 'break-words'}>
      {value}
      {contentDiffers ? (
        <Badge className="ms-1.5" size="sm" variant="warning">
          content differs
        </Badge>
      ) : null}
    </span>
  );
}

/**
 * Side-by-side decrypted diff (opening it is the explicit user action, so
 * `sync.conflict` returns both versions revealed). Plaintext lives only in
 * this dialog's state and is dropped when it closes.
 */
export function ConflictResolver({
  conflict,
  onClose
}: {
  conflict: ConflictSummary;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<ConflictDetail | null>(null);
  const [rows, setRows] = useState<DomainDiffRow[]>([]);
  const merge = useAsyncAction(detail);
  const [pick, setPick] = useState<Record<string, 'mine' | 'theirs'>>({});
  const resolve = useRpcMutation(LatchAgent.method.syncResolve, {
    invalidate: [qk.sync, qk.status, qk.entries],
    onSuccess: () => {
      notify.success(`Resolved ${conflict.name}`, 'Push to share the merge.');
      onClose();
    },
    onError: (e) => notify.error('Could not resolve', e)
  });

  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);
  useEffect(() => {
    let cancelled = false;
    void call(LatchAgent.method.syncConflict, { id: conflict.entryId })
      .then(async (d) => {
        const result = await diffRows(d);
        if (cancelled) return;
        setRows(result);
        setDetail(d);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        notify.error('Could not load conflict', e);
        onCloseRef.current();
      });
    return () => {
      cancelled = true;
      setDetail(null);
      setRows([]);
    };
  }, [conflict.entryId]);

  const changed = rows.filter((r) => r.differs);
  const fromTheirs = new Set(changed.filter((r) => pick[r.field] === 'theirs').map((r) => r.field));
  const canMerge = Boolean(detail?.mine && detail?.theirs);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogPopup className="max-w-4xl" data-testid="conflict-resolver">
        <DialogHeader>
          <DialogTitle>Resolve “{conflict.name}”</DialogTitle>
          <DialogDescription>
            This entry changed on both sides. Keep one version or pick fields from each.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel>
          {!detail ? (
            <div className="flex justify-center py-10">
              <Spinner tone="muted" />
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-36">Field</TableHead>
                  <TableHead>
                    Mine{' '}
                    {detail.mine ? (
                      <span className="font-normal text-muted-foreground">
                        · {relativeTime(detail.mine.updatedAt)}
                      </span>
                    ) : (
                      <Badge variant="error">deleted</Badge>
                    )}
                  </TableHead>
                  <TableHead>
                    Theirs{' '}
                    {detail.theirs ? (
                      <span className="font-normal text-muted-foreground">
                        · {relativeTime(detail.theirs.updatedAt)}
                      </span>
                    ) : (
                      <Badge variant="error">deleted</Badge>
                    )}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((r) => (
                  <TableRow
                    className={r.differs ? 'bg-warning/4' : undefined}
                    data-differs={r.differs || undefined}
                    key={r.field}
                  >
                    <TableCell className="align-top font-medium">{r.field}</TableCell>
                    {r.differs && canMerge ? (
                      <TableCell className="align-top" colSpan={2}>
                        <RadioGroup
                          aria-label={`Version of ${r.field}`}
                          className="grid grid-cols-2 gap-3"
                          value={pick[r.field] ?? 'mine'}
                          onValueChange={(v) =>
                            setPick((p) => ({ ...p, [r.field]: v as 'mine' | 'theirs' }))
                          }
                        >
                          <label className="flex items-start gap-2">
                            <Radio
                              aria-label={`Keep my ${r.field}`}
                              className="mt-0.5"
                              value="mine"
                            />
                            <Value
                              contentDiffers={r.contentDiffers}
                              field={r.field}
                              value={r.mine}
                            />
                          </label>
                          <label className="flex items-start gap-2">
                            <Radio
                              aria-label={`Take their ${r.field}`}
                              className="mt-0.5"
                              value="theirs"
                            />
                            <Value
                              contentDiffers={r.contentDiffers}
                              field={r.field}
                              value={r.theirs}
                            />
                          </label>
                        </RadioGroup>
                      </TableCell>
                    ) : (
                      <>
                        <TableCell className="align-top">
                          <Value contentDiffers={r.contentDiffers} field={r.field} value={r.mine} />
                        </TableCell>
                        <TableCell className="align-top">
                          <Value
                            contentDiffers={r.contentDiffers}
                            field={r.field}
                            value={r.theirs}
                          />
                        </TableCell>
                      </>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </DialogPanel>
        <DialogFooter>
          <span className="me-auto self-center text-muted-foreground text-xs">
            {changed.length} differing field{changed.length === 1 ? '' : 's'}
          </span>
          <Button
            disabled={!detail || resolve.isPending || merge.pending}
            variant="outline"
            onClick={() =>
              resolve.mutate({
                entryId: conflict.entryId,
                resolution: Resolution.MINE,
                merged: undefined
              })
            }
          >
            Keep mine
          </Button>
          <Button
            disabled={!detail || resolve.isPending || merge.pending}
            variant="outline"
            onClick={() =>
              resolve.mutate({
                entryId: conflict.entryId,
                resolution: Resolution.THEIRS,
                merged: undefined
              })
            }
          >
            Keep theirs
          </Button>
          <Button
            disabled={!detail || !canMerge || resolve.isPending || merge.pending}
            onClick={() => {
              if (!detail?.mine || !detail.theirs) return;
              const { mine, theirs } = detail;
              void merge.run(
                () => mergeEntries(mine, theirs, fromTheirs),
                (merged) => {
                  resolve.mutate({
                    entryId: conflict.entryId,
                    resolution: Resolution.MERGED,
                    merged
                  });
                },
                (error) => notify.error('Could not merge fields', error)
              );
            }}
          >
            {resolve.isPending ? <Spinner /> : null}
            Merge fields
          </Button>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}
