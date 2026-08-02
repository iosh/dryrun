import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';

import { cn } from '../lib/cn.ts';

export function TooltipProvider(
  props: ComponentPropsWithoutRef<typeof TooltipPrimitive.Provider>,
) {
  return <TooltipPrimitive.Provider {...props} />;
}

export function Tooltip(
  props: ComponentPropsWithoutRef<typeof TooltipPrimitive.Root>,
) {
  return <TooltipPrimitive.Root {...props} />;
}

export const TooltipTrigger = forwardRef<
  HTMLButtonElement,
  ComponentPropsWithoutRef<typeof TooltipPrimitive.Trigger>
>(function TooltipTrigger(props, ref) {
  return <TooltipPrimitive.Trigger ref={ref} {...props} />;
});

export const TooltipContent = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>
>(function TooltipContent({ className, sideOffset = 6, ...props }, ref) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        className={cn(
          'z-50 max-w-72 rounded-md bg-ink-950 px-2.5 py-1.5 text-xs leading-5 text-white shadow-md',
          className,
        )}
        ref={ref}
        sideOffset={sideOffset}
        {...props}
      />
    </TooltipPrimitive.Portal>
  );
});
