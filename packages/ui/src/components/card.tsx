import { mergeProps } from '@base-ui/react/merge-props';
import { useRender } from '@base-ui/react/use-render';

import { cn } from '@latch/ui/utils';

/** coss-ui Card, tuned to t3code's grouped settings surface. */
function Card({ className, render, ...props }: useRender.ComponentProps<'div'>) {
  const defaultProps = {
    className: cn(
      'relative flex flex-col rounded-2xl border border-border/60 bg-card text-card-foreground shadow-xs/5 not-dark:bg-clip-padding',
      className
    ),
    'data-slot': 'card'
  };
  return useRender({
    defaultTagName: 'div',
    props: mergeProps<'div'>(defaultProps, props),
    render
  });
}

function CardHeader({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn(
        'grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 p-6 has-data-[slot=card-action]:grid-cols-[1fr_auto]',
        className
      )}
      data-slot="card-header"
      {...props}
    />
  );
}

function CardTitle({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn('font-semibold text-lg leading-none', className)}
      data-slot="card-title"
      {...props}
    />
  );
}

function CardDescription({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn('text-muted-foreground text-sm', className)}
      data-slot="card-description"
      {...props}
    />
  );
}

function CardAction({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn(
        'col-start-2 row-span-2 row-start-1 self-start justify-self-end inline-flex',
        className
      )}
      data-slot="card-action"
      {...props}
    />
  );
}

function CardPanel({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn('p-6 in-[[data-slot=card]:has(>[data-slot=card-header])]:pt-0', className)}
      data-slot="card-panel"
      {...props}
    />
  );
}

function CardFooter({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn(
        'flex items-center p-6 in-[[data-slot=card]:has(>[data-slot=card-panel])]:pt-0',
        className
      )}
      data-slot="card-footer"
      {...props}
    />
  );
}

export {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardAction,
  CardPanel,
  CardPanel as CardContent,
  CardFooter
};
