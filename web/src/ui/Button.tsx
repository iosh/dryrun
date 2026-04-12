import type { ButtonHTMLAttributes } from 'react';

import { cn } from '../lib/cn.ts';

export interface ButtonProps
  extends Readonly<ButtonHTMLAttributes<HTMLButtonElement>> {
  variant?: 'primary' | 'secondary';
}

export function Button({
  className,
  variant = 'primary',
  type = 'button',
  ...props
}: ButtonProps) {
  return (
    <button
      className={cn(
        'inline-flex h-11 items-center justify-center rounded-[12px] px-5 text-[15px] font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-60',
        variant === 'primary'
          ? 'bg-brand-600 text-white hover:bg-brand-700'
          : 'border border-line bg-transparent text-ink-600 hover:bg-white',
        className,
      )}
      type={type}
      {...props}
    />
  );
}
