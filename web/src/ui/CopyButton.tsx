import { Check, Copy } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { cn } from '../lib/cn.ts';
import { Button } from './Button.tsx';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from './Tooltip.tsx';

export function CopyButton({
  className,
  label,
  value,
}: Readonly<{
  className?: string;
  label: string;
  value: string | (() => string);
}>) {
  const [copied, setCopied] = useState(false);
  const resetTimer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (resetTimer.current !== null) {
        window.clearTimeout(resetTimer.current);
      }
    },
    [],
  );

  async function copyValue() {
    try {
      const text = typeof value === 'function' ? value() : value;
      await navigator.clipboard.writeText(text);
    } catch {
      return;
    }

    setCopied(true);
    if (resetTimer.current !== null) {
      window.clearTimeout(resetTimer.current);
    }
    resetTimer.current = window.setTimeout(() => setCopied(false), 1400);
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          aria-label={copied ? 'Copied' : label}
          className={cn('h-7 w-7 text-ink-400', className)}
          onClick={(event) => {
            event.stopPropagation();
            void copyValue();
          }}
          size="icon"
          variant="ghost"
        >
          {copied ? (
            <Check aria-hidden="true" className="h-3.5 w-3.5 text-emerald-600" />
          ) : (
            <Copy aria-hidden="true" className="h-3.5 w-3.5" />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{copied ? 'Copied' : label}</TooltipContent>
    </Tooltip>
  );
}
