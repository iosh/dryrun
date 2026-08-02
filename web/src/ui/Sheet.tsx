import * as DialogPrimitive from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import {
  forwardRef,
  type ComponentPropsWithoutRef,
} from 'react';

import { cn } from '../lib/cn.ts';

export function Sheet(
  props: ComponentPropsWithoutRef<typeof DialogPrimitive.Root>,
) {
  return <DialogPrimitive.Root {...props} />;
}

export const SheetTrigger = forwardRef<
  HTMLButtonElement,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Trigger>
>(function SheetTrigger(props, ref) {
  return <DialogPrimitive.Trigger ref={ref} {...props} />;
});

export const SheetClose = forwardRef<
  HTMLButtonElement,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Close>
>(function SheetClose(props, ref) {
  return <DialogPrimitive.Close ref={ref} {...props} />;
});

export const SheetContent = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(function SheetContent({ children, className, ...props }, ref) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/30" />
      <DialogPrimitive.Content
        className={cn(
          'fixed inset-y-0 right-0 z-50 flex w-[min(92vw,380px)] flex-col border-l border-line bg-white shadow-xl outline-none',
          className,
        )}
        ref={ref}
        {...props}
      >
        {children}
        <DialogPrimitive.Close
          aria-label="Close"
          className="absolute right-3 top-3 flex h-9 w-9 items-center justify-center rounded-md text-ink-600 transition-colors hover:bg-shell-100 hover:text-ink-950"
        >
          <X aria-hidden="true" className="h-4 w-4" />
        </DialogPrimitive.Close>
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
});

export function SheetTitle(
  props: ComponentPropsWithoutRef<typeof DialogPrimitive.Title>,
) {
  return <DialogPrimitive.Title {...props} />;
}

export function SheetDescription(
  props: ComponentPropsWithoutRef<typeof DialogPrimitive.Description>,
) {
  return <DialogPrimitive.Description {...props} />;
}
