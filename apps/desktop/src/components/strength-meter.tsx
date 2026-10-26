import { Meter, MeterIndicator, MeterTrack } from '@latch/ui/meter';
import { useStrength } from '~/lib/strength';

export function StrengthMeter({ passphrase }: { passphrase: string }) {
  const result = useStrength(passphrase);
  const s = result?.value;
  const tone = ['bg-destructive', 'bg-destructive', 'bg-warning', 'bg-success', 'bg-success'][
    s?.score ?? 0
  ];
  return (
    <div className="flex flex-col gap-1.5" data-testid="strength-meter">
      <Meter
        aria-label="Passphrase strength"
        max={4}
        min={0}
        value={passphrase ? (s?.score ?? 0) + 0.0001 : 0}
      >
        <MeterTrack>
          <MeterIndicator className={tone} />
        </MeterTrack>
      </Meter>
      <div className="flex items-center justify-between text-xs">
        <span className="font-medium">
          {passphrase
            ? (s?.label ?? (result?.error ? 'Could not check strength' : 'Checking…'))
            : 'Strength'}
        </span>
        <span className="text-muted-foreground">{passphrase && s ? `~${s.bits} bits` : ''}</span>
      </div>
      {passphrase && s && s.hints.length > 0 ? (
        <p className="text-muted-foreground text-xs">{s.hints[0]}</p>
      ) : null}
    </div>
  );
}
