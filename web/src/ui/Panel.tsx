import type { HTMLAttributes } from 'react';

import { cn } from '../lib/cn.ts';

export interface PanelProps
  extends Readonly<HTMLAttributes<HTMLElement>> {
  as?: 'article' | 'aside' | 'section' | 'div';
}

export function Panel({
  as = 'section',
  className,
  ...props
}: PanelProps) {
  const Component = as;

  return (
    <Component
      className={cn(
        'rounded-[18px] border border-line bg-white shadow-card',
        className,
      )}
      {...props}
    />
  );
}
