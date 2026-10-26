import { Button } from '@latch/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@latch/ui/empty';
import { InputGroup, InputGroupAddon, InputGroupInput } from '@latch/ui/input-group';
import { Skeleton } from '@latch/ui/skeleton';
import { ToggleGroup, ToggleGroupItem } from '@latch/ui/toggle-group';
import { Tooltip, TooltipPopup, TooltipTrigger } from '@latch/ui/tooltip';
import { cn } from '@latch/ui/utils';
import { Link, useNavigate, useParams } from '@tanstack/react-router';
import { CopyIcon, SearchIcon, SearchXIcon, StarIcon } from 'lucide-react';
import { useRef, useState, type KeyboardEvent } from 'react';
import { EntryTypeIcon } from '~/components/icons';
import { TotpChip } from '~/components/totp-ring';
import { useCopyField } from '~/components/use-copy-field';
import { useEntries } from '~/lib/api/queries';
import { ENTRY_TYPES, ENTRY_TYPE_LABEL, PRIMARY_COPY_FIELD } from '~/lib/entries';
import { useUiStore } from '~/stores/ui';
import type { EntrySummary, EntryType } from '../../_generated/contracts';

const COPY_LABEL: Record<string, string> = {
  password: 'password',
  value: 'secret',
  body: 'note',
  privateKey: 'private key',
  key: 'API key'
};

function EntryRow({ entry, active }: { entry: EntrySummary; active: boolean }) {
  const copy = useCopyField();
  const field = PRIMARY_COPY_FIELD[entry.type];
  return (
    <li className="group/row relative" data-testid="entry-row">
      <Link
        aria-current={active ? 'page' : undefined}
        className={cn(
          'flex min-h-12 w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left outline-none transition-colors hover:bg-accent/60 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring',
          active && 'bg-accent',
          field && 'pe-10'
        )}
        data-entry-row=""
        params={{ profileId: entry.profileId, entryId: entry.id }}
        to="/p/$profileId/e/$entryId"
      >
        <span className="inline-flex size-7 shrink-0 items-center justify-center rounded-md border border-border/60 bg-card text-muted-foreground">
          <EntryTypeIcon type={entry.type} />
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="flex min-w-0 items-center gap-1">
            <span className="truncate font-medium text-sm">{entry.name}</span>
            {entry.favorite ? (
              <StarIcon
                aria-label="Favorite"
                className="size-3 shrink-0 fill-warning text-warning"
              />
            ) : null}
          </span>
          <span className="truncate text-muted-foreground text-xs">
            {ENTRY_TYPE_LABEL[entry.type]}
            {entry.subtitle ? ` · ${entry.subtitle}` : ''}
          </span>
        </span>
        {entry.hasTotp ? <TotpChip /> : null}
      </Link>
      {field ? (
        <div className="absolute inset-y-0 end-1.5 flex items-center">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  aria-label={`Copy ${COPY_LABEL[field] ?? field} of ${entry.name}`}
                  size="icon-xs"
                  variant="ghost"
                  className="opacity-60 group-hover/row:opacity-100 focus-visible:opacity-100"
                  onClick={() =>
                    copy.mutate({ id: entry.id, field, label: COPY_LABEL[field] ?? field })
                  }
                />
              }
            >
              <CopyIcon />
            </TooltipTrigger>
            <TooltipPopup>Copy {COPY_LABEL[field] ?? field}</TooltipPopup>
          </Tooltip>
        </div>
      ) : null}
    </li>
  );
}

/** Arrow-key navigation across row links (roving focus; Enter follows the link). */
function onListKeyDown(event: KeyboardEvent<HTMLElement>) {
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
  const rows = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('[data-entry-row]'));
  if (rows.length === 0) return;
  const current = rows.indexOf(document.activeElement as HTMLElement);
  let next = current;
  if (event.key === 'ArrowDown') next = current < 0 ? 0 : Math.min(rows.length - 1, current + 1);
  if (event.key === 'ArrowUp') next = current < 0 ? rows.length - 1 : Math.max(0, current - 1);
  if (event.key === 'Home') next = 0;
  if (event.key === 'End') next = rows.length - 1;
  event.preventDefault();
  rows[next]?.focus();
}

export function EntryList({ profileId, className }: { profileId: string; className?: string }) {
  const [query, setQuery] = useState('');
  const type = useUiStore((s) => s.typeFilter[profileId] ?? null);
  const setType = useUiStore((s) => s.setTypeFilter);
  const entries = useEntries({ profileId, query, type });
  const params = useParams({ strict: false }) as { entryId?: string };
  const navigate = useNavigate();
  const filterRef = useRef<HTMLInputElement>(null);

  return (
    <section aria-label="Entries" className={cn('flex min-h-0 flex-col', className)}>
      <div className="flex flex-col gap-2 border-b border-border/60 p-3">
        <InputGroup>
          <InputGroupAddon>
            <SearchIcon aria-hidden />
          </InputGroupAddon>
          <InputGroupInput
            aria-label="Filter entries"
            placeholder="Filter…"
            ref={filterRef}
            type="search"
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                (
                  e.currentTarget
                    .closest('section')
                    ?.querySelector('[data-entry-row]') as HTMLElement | null
                )?.focus();
              } else if (e.key === 'Enter') {
                const first = entries.data?.[0];
                if (first)
                  void navigate({
                    to: '/p/$profileId/e/$entryId',
                    params: { profileId, entryId: first.id }
                  });
              }
            }}
          />
        </InputGroup>
        <ToggleGroup
          aria-label="Filter by type"
          className="w-full"
          value={[type ?? 'all']}
          onValueChange={(value) => {
            const v = value[0] as EntryType | 'all' | undefined;
            if (v) setType(profileId, v === 'all' ? null : v);
          }}
        >
          <ToggleGroupItem aria-label="All types" className="min-w-0 px-1.5 text-xs" value="all">
            All
          </ToggleGroupItem>
          {ENTRY_TYPES.map((t) => (
            <Tooltip key={t}>
              <TooltipTrigger
                render={
                  <ToggleGroupItem
                    aria-label={ENTRY_TYPE_LABEL[t]}
                    className="min-w-0 flex-1 px-0"
                    value={t}
                  />
                }
              >
                <EntryTypeIcon type={t} className="size-3.5" />
              </TooltipTrigger>
              <TooltipPopup>{ENTRY_TYPE_LABEL[t]}</TooltipPopup>
            </Tooltip>
          ))}
        </ToggleGroup>
      </div>
      {/* Arrow keys delegate to native row links; the container is not an extra focus stop. */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <div
        aria-label="Entries"
        role="group"
        className="min-h-0 flex-1 overflow-y-auto p-1.5"
        onKeyDown={onListKeyDown}
      >
        {entries.isPending ? (
          <div className="flex flex-col gap-1.5 p-1" aria-busy="true">
            {Array.from({ length: 6 }, (_, i) => (
              <div className="flex items-center gap-3 px-1.5 py-1.5" key={i}>
                <Skeleton className="size-7 rounded-md" />
                <div className="flex flex-1 flex-col gap-1.5">
                  <Skeleton className="h-3.5 w-2/3" />
                  <Skeleton className="h-3 w-1/3" />
                </div>
              </div>
            ))}
          </div>
        ) : entries.data && entries.data.length > 0 ? (
          <ul aria-label="Entries" className="flex flex-col gap-0.5">
            {entries.data.map((e) => (
              <EntryRow active={e.id === params.entryId} entry={e} key={e.id} />
            ))}
          </ul>
        ) : (
          <Empty size="compact">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <SearchXIcon />
              </EmptyMedia>
              <EmptyTitle>{query || type ? 'No matches' : 'No entries yet'}</EmptyTitle>
              <EmptyDescription>
                {query || type
                  ? 'Try another filter.'
                  : 'Use “New entry” to add a login, key, .env set and more.'}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
      </div>
    </section>
  );
}
