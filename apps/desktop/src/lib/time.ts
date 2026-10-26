const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

export function relativeTime(isoString: string | null | undefined, now = Date.now()): string {
  if (!isoString) return 'never';
  const t = Date.parse(isoString);
  if (Number.isNaN(t)) return isoString;
  const diff = (t - now) / 1000;
  const abs = Math.abs(diff);
  if (abs < 45) return 'just now';
  if (abs < 3600) return rtf.format(Math.round(diff / 60), 'minute');
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), 'hour');
  if (abs < 86400 * 30) return rtf.format(Math.round(diff / 86400), 'day');
  return new Date(t).toLocaleDateString();
}

export function formatDateTime(isoString: string): string {
  const t = Date.parse(isoString);
  return Number.isNaN(t) ? isoString : new Date(t).toLocaleString();
}

export function formatDuration(totalSecs: number): string {
  const m = Math.floor(totalSecs / 60);
  const s = totalSecs % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}
