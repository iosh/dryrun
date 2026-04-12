import type { HTMLAttributes } from 'react';

import { cn } from '../lib/cn.ts';

export interface MetricCardProps
  extends Readonly<HTMLAttributes<HTMLDivElement>> {
  label: string;
  value: string;
  tone?: 'default' | 'accent';
}

export function MetricCard({
  className,
  label,
  tone = 'default',
  value,
  ...props
}: MetricCardProps) {
  return (
    <div
      className={cn(
        'flex min-h-24 flex-col gap-1 rounded-[18px] bg-shell-100 p-4',
        className,
      )}
      {...props}
    >
      <span className="font-mono text-[10px] font-medium uppercase tracking-[0.16em] text-ink-600">
        {label}
      </span>
      <span
        className={cn(
          'font-display text-[22px] font-bold',
          tone === 'accent' ? 'text-brand-600' : 'text-ink-950',
        )}
      >
        {value}
      </span>
    </div>
  );
}
