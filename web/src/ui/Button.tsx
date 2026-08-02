import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';
import {
  forwardRef,
  type ButtonHTMLAttributes,
} from 'react';

import { cn } from '../lib/cn.ts';

const buttonVariants = cva(
  'inline-flex shrink-0 items-center justify-center rounded-md text-sm font-semibold transition-colors disabled:pointer-events-none disabled:opacity-50',
  {
    defaultVariants: {
      size: 'default',
      variant: 'default',
    },
    variants: {
      size: {
        default: 'h-11 px-5',
        icon: 'h-9 w-9',
        sm: 'h-9 px-3',
      },
      variant: {
        default: 'bg-ink-950 text-white hover:bg-ink-800',
        ghost: 'text-ink-600 hover:bg-shell-100 hover:text-ink-950',
        secondary:
          'border border-line bg-white text-ink-600 hover:bg-shell-100 hover:text-ink-950',
      },
    },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      asChild = false,
      className,
      size,
      type = 'button',
      variant,
      ...props
    },
    ref,
  ) {
    const Component = asChild ? Slot : 'button';
    return (
      <Component
        className={cn(buttonVariants({ size, variant }), className)}
        ref={ref}
        type={asChild ? undefined : type}
        {...props}
      />
    );
  },
);
