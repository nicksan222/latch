import { Button } from '@latch/ui/button';
import { InputGroup, InputGroupAddon, InputGroupInput } from '@latch/ui/input-group';
import { Spinner } from '@latch/ui/spinner';
import { Textarea } from '@latch/ui/textarea';
import { Toggle } from '@latch/ui/toggle';
import { Tooltip, TooltipPopup, TooltipTrigger } from '@latch/ui/tooltip';
import { cn } from '@latch/ui/utils';
import { CopyIcon, EyeIcon, EyeOffIcon } from 'lucide-react';
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { call } from '~/lib/api/client';
import { notify } from '~/lib/notify';
import { TotpRing } from './totp-ring';
import { useCopyField } from './use-copy-field';

const MASK = '••••••••••••';

/**
 * The reveal state machine, separate from rendering so it can be unit tested:
 * reveal fetches exactly one field; the value is dropped after `hideAfterSecs`
 * or when the component unmounts (navigation).
 *
 * Every reveal gets a generation number. `hide()` and unmount bump the
 * generation, so a response that arrives late (after Hide, after a newer
 * reveal, or after unmount) is discarded instead of re-showing plaintext.
 * While a reveal is in flight further reveal requests are ignored.
 */
export function useReveal<T = string>(fetchValue: () => Promise<T>, hideAfterSecs: number) {
  const [value, setValue] = useState<T | null>(null);
  const [loading, setLoading] = useState(false);
  const [secondsLeft, setSecondsLeft] = useState(0);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);
  const generation = useRef(0);
  const inFlight = useRef<number | null>(null);
  const mounted = useRef(false);
  const fetchRef = useRef(fetchValue);
  useEffect(() => {
    fetchRef.current = fetchValue;
  }, [fetchValue]);

  const clear = useCallback(() => {
    if (timer.current) clearInterval(timer.current);
    timer.current = null;
  }, []);

  const hide = useCallback(() => {
    generation.current += 1; // invalidates any in-flight reveal
    inFlight.current = null;
    clear();
    setValue(null);
    setSecondsLeft(0);
    setLoading(false);
  }, [clear]);

  const reveal = useCallback(async () => {
    if (inFlight.current !== null) return; // no duplicate pending reveals
    const gen = ++generation.current;
    inFlight.current = gen;
    setLoading(true);
    try {
      const v = await fetchRef.current();
      if (gen !== generation.current || !mounted.current) return; // stale: hidden, superseded or unmounted
      setValue(v);
      clear();
      if (hideAfterSecs > 0) {
        const deadline = Date.now() + hideAfterSecs * 1000;
        setSecondsLeft(hideAfterSecs);
        timer.current = setInterval(() => {
          const left = Math.ceil((deadline - Date.now()) / 1000);
          if (left <= 0) hide();
          else setSecondsLeft(left);
        }, 250);
      }
    } catch (error) {
      if (gen === generation.current && mounted.current) notify.error('Could not reveal', error);
    } finally {
      if (inFlight.current === gen) {
        inFlight.current = null;
        // This is already inside finally; stale/unmounted reveals must not update state.
        // react-doctor-disable-next-line no-loading-flag-reset-outside-finally
        if (mounted.current) setLoading(false);
      }
    }
  }, [clear, hide, hideAfterSecs]);

  /** Replace the shown value (e.g. TOTP rotation) only while still revealed. */
  const update = useCallback((next: T) => setValue((prev) => (prev === null ? null : next)), []);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current += 1;
      inFlight.current = null;
      clear();
    };
  }, [clear]);

  return { value, revealed: value !== null, loading, secondsLeft, reveal, hide, update };
}

export interface SecretFieldProps {
  entryId: string;
  /** Secret path, e.g. `password`, `vars.API_KEY`, `custom.Recovery`. */
  field: string;
  label: string;
  hideAfterSecs: number;
  multiline?: boolean;
  /** Field has no stored value: render a dash, no actions. */
  empty?: boolean;
  className?: string;
  /** Hide the visible label (tables). */
  compact?: boolean;
}

/**
 * Secret field composite (SPEC §9.6): masked input, reveal toggle (Space),
 * copy button (Enter) via `clipboard.copy`. Plaintext lives only in local
 * state and is dropped on timeout or unmount.
 */
export function SecretField({
  entryId,
  field,
  label,
  hideAfterSecs,
  multiline,
  empty,
  className,
  compact
}: SecretFieldProps) {
  const id = useId();
  const copy = useCopyField();
  const { value, revealed, loading, secondsLeft, reveal, hide } = useReveal(
    async () => (await call('entries.reveal', { id: entryId, field })).value,
    hideAfterSecs
  );

  if (empty) {
    return (
      <div className={cn('flex flex-col gap-1.5', className)}>
        {!compact && <span className="font-medium text-muted-foreground text-xs">{label}</span>}
        <span className="text-muted-foreground text-sm">—</span>
      </div>
    );
  }

  const actions = (
    <SecretActions
      revealed={revealed}
      loading={loading}
      secondsLeft={secondsLeft}
      label={label}
      reveal={reveal}
      hide={hide}
      onCopy={() => copy.mutate({ id: entryId, field, label })}
    />
  );

  return (
    <div className={cn('flex min-w-0 flex-col gap-1.5', className)} data-testid={`secret-${field}`}>
      {!compact && (
        <label className="font-medium text-muted-foreground text-xs" htmlFor={id}>
          {label}
        </label>
      )}
      {multiline && revealed ? (
        <div className="flex flex-col gap-1.5">
          <Textarea
            aria-label={label}
            className="font-mono text-xs"
            id={id}
            readOnly
            rows={6}
            value={value ?? ''}
          />
          <div className="flex items-center justify-end gap-1">{actions}</div>
        </div>
      ) : (
        <InputGroup>
          <InputGroupInput
            aria-label={label}
            className="font-mono"
            id={id}
            readOnly
            data-revealed={revealed || undefined}
            value={revealed ? (value ?? '') : MASK}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                copy.mutate({ id: entryId, field, label });
              } else if (event.key === ' ') {
                event.preventDefault();
                if (revealed || loading) hide();
                else void reveal();
              }
            }}
          />
          <InputGroupAddon align="inline-end" className="gap-0.5">
            {actions}
          </InputGroupAddon>
        </InputGroup>
      )}
    </div>
  );
}

function SecretActions({
  revealed,
  loading,
  secondsLeft,
  label,
  reveal,
  hide,
  onCopy
}: {
  revealed: boolean;
  loading: boolean;
  secondsLeft: number;
  label: string;
  reveal: () => Promise<void>;
  hide: () => void;
  onCopy: () => void;
}) {
  return (
    <>
      {revealed && secondsLeft > 0 ? (
        <span className="px-1 text-2xs tabular-nums text-muted-foreground" aria-live="polite">
          {secondsLeft}s
        </span>
      ) : null}
      <Tooltip>
        <TooltipTrigger
          render={
            <Toggle
              aria-label={revealed || loading ? `Hide ${label}` : `Reveal ${label}`}
              pressed={revealed || loading}
              onPressedChange={(pressed) => (pressed ? void reveal() : hide())}
              size="sm"
              className="size-7 sm:size-6"
            />
          }
        >
          {loading ? <Spinner size="sm" /> : revealed ? <EyeOffIcon /> : <EyeIcon />}
        </TooltipTrigger>
        <TooltipPopup>{revealed ? 'Hide' : 'Reveal'}</TooltipPopup>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger
          render={
            <Button aria-label={`Copy ${label}`} size="icon-xs" variant="ghost" onClick={onCopy} />
          }
        >
          <CopyIcon />
        </TooltipTrigger>
        <TooltipPopup>Copy (clears automatically)</TooltipPopup>
      </Tooltip>
    </>
  );
}

/** TOTP field: ring countdown, masked code with reveal (refreshes on rotation), copy. */
export function TotpField({
  entryId,
  period,
  hideAfterSecs
}: {
  entryId: string;
  period: number;
  hideAfterSecs: number;
}) {
  const copy = useCopyField();
  const {
    value: codeInfo,
    revealed,
    loading,
    reveal,
    hide,
    update,
    secondsLeft
  } = useReveal(async () => {
    const r = await call('totp.get', { id: entryId });
    return { code: r.code, expiresAt: Date.now() + r.expiresIn * 1000 };
  }, hideAfterSecs);

  // Refresh the code when it rotates while revealed. A response landing after
  // Hide/unmount is dropped (cancelled flag + `update` only while revealed).
  useEffect(() => {
    if (!revealed || !codeInfo) return;
    let cancelled = false;
    const t = setTimeout(
      () => {
        void call('totp.get', { id: entryId }).then(
          (r) => {
            if (!cancelled) update({ code: r.code, expiresAt: Date.now() + r.expiresIn * 1000 });
          },
          () => {
            if (!cancelled) hide();
          }
        );
      },
      Math.max(250, codeInfo.expiresAt - Date.now() + 50)
    );
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [revealed, codeInfo, entryId, hide, update]);

  const shown =
    revealed && codeInfo
      ? `${codeInfo.code.slice(0, Math.ceil(codeInfo.code.length / 2))} ${codeInfo.code.slice(Math.ceil(codeInfo.code.length / 2))}`
      : '••• •••';

  return (
    <div className="flex min-w-0 flex-col gap-1.5" data-testid="totp-field">
      <span className="font-medium text-muted-foreground text-xs">One-time code</span>
      <InputGroup>
        <InputGroupAddon>
          <TotpRing period={period} />
        </InputGroupAddon>
        <InputGroupInput
          aria-label="One-time code"
          className="font-mono tracking-widest"
          readOnly
          value={shown}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              copy.mutate({ id: entryId, field: 'totp', label: 'one-time code' });
            } else if (event.key === ' ') {
              event.preventDefault();
              if (revealed || loading) hide();
              else void reveal();
            }
          }}
        />
        <InputGroupAddon align="inline-end" className="gap-0.5">
          {revealed && secondsLeft > 0 ? (
            <span className="px-1 text-2xs tabular-nums text-muted-foreground">{secondsLeft}s</span>
          ) : null}
          <Toggle
            aria-label={revealed || loading ? 'Hide one-time code' : 'Reveal one-time code'}
            pressed={revealed || loading}
            onPressedChange={(pressed) => (pressed ? void reveal() : hide())}
            size="sm"
            className="size-7 sm:size-6"
          >
            {loading ? <Spinner size="sm" /> : revealed ? <EyeOffIcon /> : <EyeIcon />}
          </Toggle>
          <Button
            aria-label="Copy one-time code"
            size="icon-xs"
            variant="ghost"
            onClick={() => copy.mutate({ id: entryId, field: 'totp', label: 'one-time code' })}
          >
            <CopyIcon />
          </Button>
        </InputGroupAddon>
      </InputGroup>
    </div>
  );
}
