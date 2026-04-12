import type { HTMLAttributes } from 'react';

import { cn } from '../lib/cn.ts';

const toneClassNames = {
  amber: 'bg-amber-100 text-amber-700',
  blue: 'bg-blue-100 text-blue-700',
  green: 'bg-emerald-100 text-emerald-700',
  slate: 'bg-slate-200 text-slate-600',
  violet: 'bg-violet-100 text-violet-700',
} as const;

export interface BadgeProps
  extends Readonly<HTMLAttributes<HTMLSpanElement>> {
  tone?: keyof typeof toneClassNames;
}

export function Badge({
  children,
  className,
  tone = 'blue',
  ...props
}: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-[999px] px-2.5 py-1 font-mono text-[10px] font-semibold uppercase tracking-[0.16em]',
        toneClassNames[tone],
        className,
      )}
      {...props}
    >
      {children}
    </span>
  );
}
