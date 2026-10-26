import { useNow } from '@latch/ui/hooks/use-now';
import { cn } from '@latch/ui/utils';
import { totpSecondsLeft } from '~/lib/totp';

/**
 * Circular TOTP countdown. Computed from the period and the clock only, so it
 * needs no secret. Exposed as an ARIA meter.
 */
export function TotpRing({
  period = 30,
  size = 18,
  className
}: {
  period?: number;
  size?: number;
  className?: string;
}) {
  const now = useNow(1000);
  const left = totpSecondsLeft(period, now);
  const r = (size - 3) / 2;
  const c = 2 * Math.PI * r;
  const fraction = left / period;
  const urgent = left <= 5;
  return (
    <span
      aria-label={`${left} seconds until the code changes`}
      aria-valuemax={period}
      aria-valuemin={0}
      aria-valuenow={left}
      className={cn('inline-flex items-center gap-1 tabular-nums', className)}
      role="meter"
    >
      <svg
        aria-hidden
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        width={size}
        className="-rotate-90"
      >
        <circle
          cx={size / 2}
          cy={size / 2}
          fill="none"
          r={r}
          className="stroke-input"
          strokeWidth={2.5}
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          fill="none"
          r={r}
          className={cn(
            'transition-[stroke-dashoffset] duration-1000 ease-linear motion-reduce:transition-none',
            urgent ? 'stroke-destructive' : 'stroke-primary'
          )}
          strokeDasharray={c}
          strokeDashoffset={c * (1 - fraction)}
          strokeLinecap="round"
          strokeWidth={2.5}
        />
      </svg>
    </span>
  );
}

/** Small chip for list rows: ring + seconds left. */
export function TotpChip({ period = 30 }: { period?: number }) {
  const now = useNow(1000);
  const left = totpSecondsLeft(period, now);
  return (
    <span
      className={cn(
        'inline-flex h-5 items-center gap-1 rounded-sm bg-muted px-1 text-2xs tabular-nums text-muted-foreground',
        left <= 5 && 'text-destructive-foreground'
      )}
      title="TOTP code rotates"
    >
      <TotpRing period={period} size={12} />
      {left}s
    </span>
  );
}
