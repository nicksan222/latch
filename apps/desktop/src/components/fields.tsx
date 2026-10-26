import { Button } from '@latch/ui/button';
import { Input, type InputProps } from '@latch/ui/input';
import { InputGroup, InputGroupAddon, InputGroupInput } from '@latch/ui/input-group';
import { Tooltip, TooltipPopup, TooltipTrigger } from '@latch/ui/tooltip';
import { cn } from '@latch/ui/utils';
import { CopyIcon, DicesIcon, EyeIcon, EyeOffIcon } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useAsyncAction } from '~/lib/async-ui';
import { notify } from '~/lib/notify';
import { generatePassword } from '~/lib/password';

/** Label + control + error, wired with ids for a11y. */
export function FormField({
  label,
  error,
  description,
  children,
  className
}: {
  label: ReactNode;
  error?: string | undefined;
  description?: ReactNode;
  children: (props: {
    id: string;
    'aria-invalid'?: true;
    'aria-describedby'?: string;
  }) => ReactNode;
  className?: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;
  const descId = `${id}-desc`;
  const describedBy = [error ? errorId : null, description ? descId : null]
    .filter(Boolean)
    .join(' ');
  return (
    <div className={cn('flex min-w-0 flex-col gap-1.5', className)}>
      <label className="font-medium text-foreground text-sm" htmlFor={id}>
        {label}
      </label>
      {children({
        id,
        ...(error ? { 'aria-invalid': true as const } : {}),
        ...(describedBy ? { 'aria-describedby': describedBy } : {})
      })}
      {description ? (
        <p className="text-muted-foreground text-xs leading-relaxed" id={descId}>
          {description}
        </p>
      ) : null}
      {error ? (
        <p className="text-destructive-foreground text-xs" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

/** Editable password input with show toggle and optional generator. */
export function PasswordInput({
  value,
  onChange,
  generate,
  className,
  ...props
}: Omit<InputProps, 'value' | 'onChange' | 'type' | 'className'> & {
  className?: string;
  value: string;
  onChange: (value: string) => void;
  generate?: boolean;
}) {
  const [shown, setShown] = useState(false);
  const generation = useAsyncAction(value);
  return (
    <InputGroup className={className}>
      <InputGroupInput
        {...props}
        autoComplete="off"
        className="font-mono"
        spellCheck={false}
        type={shown ? 'text' : 'password'}
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
      />
      <InputGroupAddon align="inline-end" className="gap-0.5">
        {generate ? (
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  aria-label="Generate password"
                  disabled={generation.pending}
                  size="icon-xs"
                  variant="ghost"
                  onClick={() => {
                    void generation.run(
                      generatePassword,
                      (password) => {
                        onChange(password);
                        setShown(true);
                      },
                      (error) => notify.error('Could not generate password', error)
                    );
                  }}
                />
              }
            >
              <DicesIcon />
            </TooltipTrigger>
            <TooltipPopup>Generate</TooltipPopup>
          </Tooltip>
        ) : null}
        <Button
          aria-label={shown ? 'Hide value' : 'Show value'}
          aria-pressed={shown}
          size="icon-xs"
          variant="ghost"
          onClick={() => setShown((s) => !s)}
        >
          {shown ? <EyeOffIcon /> : <EyeIcon />}
        </Button>
      </InputGroupAddon>
    </InputGroup>
  );
}

/** Read-only non-secret value with an optional copy action. */
export function PlainField({
  label,
  value,
  onCopy,
  mono,
  children
}: {
  label: string;
  value?: string | null;
  onCopy?: () => void;
  mono?: boolean;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <span className="font-medium text-muted-foreground text-xs">{label}</span>
      {children ?? (
        <div className="flex min-h-8 min-w-0 items-center gap-1">
          <span
            className={cn(
              'min-w-0 flex-1 truncate text-sm',
              mono && 'font-mono text-xs',
              !value && 'text-muted-foreground'
            )}
          >
            {value || '—'}
          </span>
          {onCopy && value ? (
            <Button aria-label={`Copy ${label}`} size="icon-xs" variant="ghost" onClick={onCopy}>
              <CopyIcon />
            </Button>
          ) : null}
        </div>
      )}
    </div>
  );
}

export { Input };
