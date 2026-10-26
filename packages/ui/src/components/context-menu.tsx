import { ContextMenu as ContextMenuPrimitive } from '@base-ui/react/context-menu';

import { cn } from '@latch/ui/utils';

/** coss-ui Context Menu: Base UI context menu sharing the Menu popup styling. */
const ContextMenu = ContextMenuPrimitive.Root;

function ContextMenuTrigger({ className, ...props }: ContextMenuPrimitive.Trigger.Props) {
  return (
    <ContextMenuPrimitive.Trigger
      className={cn('select-none', className)}
      data-slot="context-menu-trigger"
      {...props}
    />
  );
}

function ContextMenuPopup({ className, children, ...props }: ContextMenuPrimitive.Popup.Props) {
  return (
    <ContextMenuPrimitive.Portal>
      <ContextMenuPrimitive.Positioner className="z-[130]" data-slot="context-menu-positioner">
        <ContextMenuPrimitive.Popup
          className={cn(
            'dropdown-glass relative flex min-w-[min(11rem,calc(100vw-2rem))] origin-(--transform-origin) rounded-lg shadow-[0_16px_40px_-18px_rgb(0_0_0/55%)] outline-none dark:shadow-[0_18px_44px_-18px_rgb(0_0_0/80%)]',
            className
          )}
          data-slot="menu-popup"
          {...props}
        >
          <div className="max-h-(--available-height) w-full overflow-y-auto p-1">{children}</div>
        </ContextMenuPrimitive.Popup>
      </ContextMenuPrimitive.Positioner>
    </ContextMenuPrimitive.Portal>
  );
}

export { ContextMenu, ContextMenuTrigger, ContextMenuPopup };
