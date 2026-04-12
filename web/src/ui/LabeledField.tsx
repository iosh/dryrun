import type { ReactNode } from 'react';

import { cn } from '../lib/cn.ts';

export interface LabeledFieldProps {
  label: string;
  optional?: boolean;
  className?: string;
  children: ReactNode;
}

export function LabeledField({
  children,
  className,
  label,
  optional = false,
}: Readonly<LabeledFieldProps>) {
  return (
    <label className={cn('flex w-full flex-col gap-2', className)}>
      <span className="font-mono text-[11px] font-medium uppercase tracking-[0.12em] text-ink-600">
        {label}
        {optional ? ' (Optional)' : ''}
      </span>
      {children}
    </label>
  );
}
