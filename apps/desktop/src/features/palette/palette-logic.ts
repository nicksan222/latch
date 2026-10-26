import { PRIMARY_COPY_FIELD } from '~/lib/entries';
import type { EntrySummary } from '../../_generated/contracts';

export type PaletteAction =
  | { kind: 'copy'; field: string; label: string }
  | { kind: 'open' }
  | { kind: 'none'; reason: string };

const FIELD_LABEL: Record<string, string> = {
  password: 'password',
  value: 'secret',
  body: 'note',
  privateKey: 'private key',
  key: 'API key',
  totp: 'one-time code'
};

/**
 * SPEC §9.3 palette keys: Enter copies the password (primary secret),
 * Shift+Enter copies the TOTP code, Ctrl/Cmd+Enter opens the entry.
 */
export function paletteAction(
  key: { key: string; shiftKey?: boolean; ctrlKey?: boolean; metaKey?: boolean },
  item: EntrySummary
): PaletteAction | null {
  if (key.key !== 'Enter') return null;
  if (key.ctrlKey || key.metaKey) return { kind: 'open' };
  if (key.shiftKey) {
    return item.hasTotp
      ? { kind: 'copy', field: 'totp', label: FIELD_LABEL.totp! }
      : { kind: 'none', reason: `${item.name} has no one-time code` };
  }
  const field = PRIMARY_COPY_FIELD[item.type];
  if (!field) return { kind: 'open' };
  return { kind: 'copy', field, label: FIELD_LABEL[field] ?? field };
}
