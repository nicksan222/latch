import type { EntrySummary } from '@latch/rpc/v1';
import {
  Command,
  CommandDialog,
  CommandDialogPopup,
  CommandFooter,
  CommandInput,
  CommandItem,
  CommandList,
  CommandPanel
} from '@latch/ui/command';
import { Kbd, KbdGroup } from '@latch/ui/kbd';
import { Spinner } from '@latch/ui/spinner';
import { useNavigate } from '@tanstack/react-router';
import { useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { EntryTypeIcon, ProfileDot } from '~/components/icons';
import { TotpChip } from '~/components/totp-ring';
import { useCopyField } from '~/components/use-copy-field';
import { MOD_KEY } from '~/features/shell/app-header';
import { useEntries, useProfiles } from '~/lib/api/queries';
import { ENTRY_TYPE_LABEL } from '~/lib/entries';
import { notify } from '~/lib/notify';
import { useUiStore } from '~/stores/ui';
import { paletteAction } from './palette-logic';

export function CommandPalette() {
  const open = useUiStore((s) => s.paletteOpen);
  const setOpen = useUiStore((s) => s.setPaletteOpen);
  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandDialogPopup
        aria-label="Quick search"
        className="overflow-hidden"
        data-testid="command-palette"
      >
        {open ? <PaletteBody onClose={() => setOpen(false)} /> : null}
      </CommandDialogPopup>
    </CommandDialog>
  );
}

export function PaletteBody({ onClose }: { onClose: () => void }) {
  const [query, setQuery] = useState('');
  const highlighted = useRef<EntrySummary | undefined>(undefined);
  const entries = useEntries({ query });
  const profiles = useProfiles();
  const navigate = useNavigate();
  const copy = useCopyField();
  const items = entries.data ?? [];
  const profileById = useMemo(
    () => new Map((profiles.data ?? []).map((p) => [p.id, p])),
    [profiles.data]
  );

  const run = (
    item: EntrySummary,
    key: { key: string; shiftKey?: boolean; ctrlKey?: boolean; metaKey?: boolean }
  ) => {
    const action = paletteAction(key, item);
    if (!action) return false;
    if (action.kind === 'none') {
      notify.info(action.reason);
      return true;
    }
    if (action.kind === 'open') {
      onClose();
      void navigate({
        to: '/p/$profileId/e/$entryId',
        params: { profileId: item.profileId, entryId: item.id }
      });
      return true;
    }
    copy.mutate({ id: item.id, field: action.field, label: action.label });
    onClose();
    return true;
  };

  const onKeyDown = (
    event: KeyboardEvent<HTMLInputElement> & { preventBaseUIHandler?: () => void }
  ) => {
    if (event.key !== 'Enter') return;
    const item = items.find((item) => item.id === highlighted.current?.id) ?? items[0];
    if (!item) return;
    if (run(item, event)) {
      event.preventDefault();
      event.preventBaseUIHandler?.();
    }
  };

  return (
    <Command
      filter={null}
      items={items}
      itemToStringValue={(item) => (item as EntrySummary).name}
      value={query}
      onValueChange={setQuery}
      onItemHighlighted={(item) => {
        highlighted.current = item as EntrySummary | undefined;
      }}
    >
      <CommandInput
        aria-label="Search entries"
        placeholder="Search entries across profiles…"
        onKeyDown={onKeyDown}
      />
      <CommandPanel className="flex max-h-[min(28rem,70vh)] min-h-0 flex-col">
        {entries.isPending ? (
          <div className="flex justify-center py-10">
            <Spinner tone="muted" />
          </div>
        ) : items.length === 0 ? (
          <div className="py-10 text-center text-muted-foreground text-sm">
            No matching entries.
          </div>
        ) : (
          <CommandList>
            {(item: EntrySummary) => {
              const profile = profileById.get(item.profileId);
              return (
                <CommandItem
                  key={item.id}
                  value={item}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => run(item, { key: 'Enter', ctrlKey: true })}
                  data-testid="palette-item"
                >
                  <EntryTypeIcon className="text-muted-foreground" type={item.type} />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-foreground text-sm">{item.name}</span>
                    <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground/70 text-xs">
                      {profile ? <ProfileDot color={profile.color} /> : null}
                      <span className="truncate">
                        {profile?.name ?? ''} · {ENTRY_TYPE_LABEL[item.type]}
                        {item.subtitle ? ` · ${item.subtitle}` : ''}
                      </span>
                    </span>
                  </span>
                  {item.hasTotp ? <TotpChip /> : null}
                </CommandItem>
              );
            }}
          </CommandList>
        )}
      </CommandPanel>
      <CommandFooter className="max-sm:flex-col max-sm:items-start">
        <div className="flex flex-wrap items-center gap-3">
          <KbdGroup>
            <Kbd>Enter</Kbd>
            <span>Copy password</span>
          </KbdGroup>
          <KbdGroup>
            <Kbd>⇧ Enter</Kbd>
            <span>Copy TOTP</span>
          </KbdGroup>
          <KbdGroup>
            <Kbd>{MOD_KEY} Enter</Kbd>
            <span>Open</span>
          </KbdGroup>
          <KbdGroup>
            <Kbd>Esc</Kbd>
            <span>Close</span>
          </KbdGroup>
        </div>
      </CommandFooter>
    </Command>
  );
}
