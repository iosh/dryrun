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
      <span className="text-xs font-medium text-ink-600">
        {label}
        {optional ? <span className="font-normal text-ink-400"> · Optional</span> : null}
      </span>
      {children}
    </label>
  );
}
