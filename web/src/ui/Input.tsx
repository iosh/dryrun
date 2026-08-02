import { forwardRef, type InputHTMLAttributes } from 'react';

import { cn } from '../lib/cn.ts';

export const Input = forwardRef<
  HTMLInputElement,
  InputHTMLAttributes<HTMLInputElement>
>(function Input({ className, type = 'text', ...props }, ref) {
  return (
    <input
      className={cn(
        'h-11 w-full rounded-md border border-line bg-white px-3 text-sm text-ink-950 outline-none transition-colors placeholder:text-ink-400 focus:border-brand-600 focus:ring-2 focus:ring-brand-600/15 disabled:cursor-not-allowed disabled:bg-shell-100 disabled:text-ink-400',
        className,
      )}
      ref={ref}
      type={type}
      {...props}
    />
  );
});
