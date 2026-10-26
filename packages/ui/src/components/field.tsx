import { Field as FieldPrimitive } from '@base-ui/react/field';
import { Fieldset as FieldsetPrimitive } from '@base-ui/react/fieldset';

import { cn } from '@latch/ui/utils';

/** coss-ui Field (Base UI): label + control + description + error. */
function Field({ className, ...props }: FieldPrimitive.Root.Props) {
  return (
    <FieldPrimitive.Root
      className={cn('flex min-w-0 flex-col items-start gap-2', className)}
      data-slot="field"
      {...props}
    />
  );
}

function FieldLabel({ className, ...props }: FieldPrimitive.Label.Props) {
  return (
    <FieldPrimitive.Label
      className={cn(
        'inline-flex items-center gap-2 font-medium text-base/4.5 text-foreground sm:text-sm/4',
        className
      )}
      data-slot="field-label"
      {...props}
    />
  );
}

function FieldDescription({ className, ...props }: FieldPrimitive.Description.Props) {
  return (
    <FieldPrimitive.Description
      className={cn('text-muted-foreground text-xs', className)}
      data-slot="field-description"
      {...props}
    />
  );
}

/** Always-rendered error text (we validate ourselves rather than via constraint validation). */
function FieldError({ className, children, ...props }: React.ComponentProps<'p'>) {
  if (!children) return null;
  return (
    <p
      className={cn('text-destructive-foreground text-xs', className)}
      data-slot="field-error"
      role="alert"
      {...props}
    >
      {children}
    </p>
  );
}

const FieldControl = FieldPrimitive.Control;

function Fieldset({ className, ...props }: FieldsetPrimitive.Root.Props) {
  return (
    <FieldsetPrimitive.Root
      className={cn('flex w-full flex-col gap-4', className)}
      data-slot="fieldset"
      {...props}
    />
  );
}

function FieldsetLegend({ className, ...props }: FieldsetPrimitive.Legend.Props) {
  return (
    <FieldsetPrimitive.Legend
      className={cn('font-semibold text-sm text-foreground', className)}
      data-slot="fieldset-legend"
      {...props}
    />
  );
}

export { Field, FieldLabel, FieldDescription, FieldError, FieldControl, Fieldset, FieldsetLegend };
