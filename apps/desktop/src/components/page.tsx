import { cn } from '@latch/ui/utils';
import type { ComponentPropsWithoutRef, ReactNode } from 'react';
import { useId } from 'react';

/** Topbar of a workspace page (mirrors t3code's WorkspacePageHeader). */
export function PageHeader({ className, ...props }: ComponentPropsWithoutRef<'header'>) {
  return (
    <header
      className={cn(
        'flex h-(--workspace-topbar-height) min-h-(--workspace-topbar-height) shrink-0 items-center gap-2 border-b border-border/60 px-3 sm:px-4',
        className
      )}
      {...props}
    />
  );
}

export function PageTitle({ className, children, ...props }: ComponentPropsWithoutRef<'h1'>) {
  return (
    <h1
      className={cn('min-w-0 truncate font-medium text-sm text-foreground', className)}
      {...props}
    >
      {children}
    </h1>
  );
}

/** Shared content rhythm for detail panes and full pages, without owning scroll. */
export function PageContent({
  width = 'readable',
  className,
  ...props
}: ComponentPropsWithoutRef<'div'> & { width?: 'readable' | 'wide' }) {
  return (
    <div
      className={cn(
        '@container/page-content mx-auto flex w-full min-w-0 flex-col gap-6 px-5 pt-6 pb-12 sm:px-6',
        width === 'readable' ? 'max-w-3xl' : 'max-w-5xl',
        className
      )}
      {...props}
    />
  );
}

export function ContentTitle({ className, children, ...props }: ComponentPropsWithoutRef<'h2'>) {
  return (
    <h2
      className={cn(
        'min-w-0 break-words font-semibold text-lg leading-6 tracking-tight',
        className
      )}
      {...props}
    >
      {children}
    </h2>
  );
}

/** Scrollable page body with t3code's readable width. */
export function PageContainer({
  width = 'readable',
  className,
  ...props
}: ComponentPropsWithoutRef<'div'> & { width?: 'readable' | 'wide' }) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <PageContent width={width} className={className} {...props} />
    </div>
  );
}

export function SettingsGroup({ className, ...props }: ComponentPropsWithoutRef<'div'>) {
  return (
    <div
      className={cn(
        'relative overflow-visible rounded-xl border border-border/60 bg-card/40 text-foreground shadow-xs/5 [&>*+*]:border-t [&>*+*]:border-border/50',
        className
      )}
      {...props}
    />
  );
}

/** A titled group of rows (t3code SettingsSection). */
export function SettingsSection({
  title,
  icon,
  headerAction,
  children,
  className,
  ...props
}: Omit<ComponentPropsWithoutRef<'section'>, 'title'> & {
  title: ReactNode;
  icon?: ReactNode;
  headerAction?: ReactNode;
}) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className={cn('space-y-2.5', className)} {...props}>
      <div className="flex min-h-7 items-center justify-between gap-4 px-3 sm:px-4">
        <h2
          id={headingId}
          className="flex min-h-7 items-center gap-2 font-medium text-muted-foreground text-sm"
        >
          {icon}
          {title}
        </h2>
        {headerAction ? <div className="flex items-center gap-2">{headerAction}</div> : null}
      </div>
      <SettingsGroup>{children}</SettingsGroup>
    </section>
  );
}

/** Title/description on the left, control on the right (t3code SettingsRow). */
export function SettingsRow({
  title,
  description,
  control,
  children,
  className,
  htmlFor
}: {
  title: ReactNode;
  description?: ReactNode;
  control?: ReactNode;
  children?: ReactNode;
  className?: string;
  htmlFor?: string;
}) {
  return (
    <div
      className={cn(
        '@container/settings-row px-3 sm:px-4',
        children ? 'pt-3 pb-3' : 'py-3',
        className
      )}
      data-slot="settings-row"
    >
      <div className="flex flex-col gap-3 @min-[32rem]/settings-row:grid @min-[32rem]/settings-row:grid-cols-[minmax(0,1fr)_minmax(10rem,auto)] @min-[32rem]/settings-row:items-center @min-[32rem]/settings-row:gap-8">
        <div className="min-w-0 flex-1 space-y-1">
          {htmlFor ? (
            <label className="font-medium text-foreground text-sm" htmlFor={htmlFor}>
              {title}
            </label>
          ) : (
            <h3 className="font-medium text-foreground text-sm">{title}</h3>
          )}
          {description ? (
            <p className="max-w-xl text-muted-foreground text-xs leading-relaxed">{description}</p>
          ) : null}
        </div>
        {control ? (
          <div className="flex w-full min-w-0 shrink-0 items-center gap-2 @min-[32rem]/settings-row:w-auto @min-[32rem]/settings-row:justify-end">
            {control}
          </div>
        ) : null}
      </div>
      {children ? <div className="mt-3">{children}</div> : null}
    </div>
  );
}
