import * as SelectPrimitive from '@radix-ui/react-select';
import { Check, ChevronDown, ChevronUp } from 'lucide-react';
import {
  forwardRef,
  type ComponentPropsWithoutRef,
} from 'react';

import { cn } from '../lib/cn.ts';

export function Select(
  props: ComponentPropsWithoutRef<typeof SelectPrimitive.Root>,
) {
  return <SelectPrimitive.Root {...props} />;
}

export function SelectValue(
  props: ComponentPropsWithoutRef<typeof SelectPrimitive.Value>,
) {
  return <SelectPrimitive.Value {...props} />;
}

export const SelectTrigger = forwardRef<
  HTMLButtonElement,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Trigger>
>(function SelectTrigger({ children, className, ...props }, ref) {
  return (
    <SelectPrimitive.Trigger
      className={cn(
        'flex h-11 w-full items-center justify-between gap-3 rounded-md border border-line bg-white px-3 text-sm text-ink-950 outline-none transition-colors data-placeholder:text-ink-400 focus:border-brand-600 focus:ring-2 focus:ring-brand-600/15 disabled:cursor-not-allowed disabled:bg-shell-100 disabled:text-ink-400',
        className,
      )}
      ref={ref}
      {...props}
    >
      {children}
      <SelectPrimitive.Icon asChild>
        <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0 text-ink-600" />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
});

export const SelectContent = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Content>
>(function SelectContent({ children, className, position = 'popper', ...props }, ref) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        className={cn(
          'z-50 max-h-72 min-w-(--radix-select-trigger-width) overflow-hidden rounded-md border border-line bg-white text-ink-950 shadow-lg',
          position === 'popper' && 'translate-y-1',
          className,
        )}
        position={position}
        ref={ref}
        {...props}
      >
        <SelectPrimitive.ScrollUpButton className="flex h-7 items-center justify-center text-ink-600">
          <ChevronUp aria-hidden="true" className="h-4 w-4" />
        </SelectPrimitive.ScrollUpButton>
        <SelectPrimitive.Viewport className="p-1">
          {children}
        </SelectPrimitive.Viewport>
        <SelectPrimitive.ScrollDownButton className="flex h-7 items-center justify-center text-ink-600">
          <ChevronDown aria-hidden="true" className="h-4 w-4" />
        </SelectPrimitive.ScrollDownButton>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
});

export const SelectItem = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Item>
>(function SelectItem({ children, className, ...props }, ref) {
  return (
    <SelectPrimitive.Item
      className={cn(
        'relative flex min-h-9 cursor-default select-none items-center rounded-sm py-2 pl-8 pr-3 text-sm outline-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-shell-100',
        className,
      )}
      ref={ref}
      {...props}
    >
      <span className="absolute left-2 flex h-4 w-4 items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <Check aria-hidden="true" className="h-4 w-4" />
        </SelectPrimitive.ItemIndicator>
      </span>
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
  );
});
