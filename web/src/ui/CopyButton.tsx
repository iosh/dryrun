import { Check, Copy } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { cn } from '../lib/cn.ts';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from './Tooltip.tsx';

const toneClassNames = {
  code: 'bg-code text-code-ink hover:bg-white/10 hover:text-white',
  default: 'text-ink-400 hover:bg-shell-100 hover:text-ink-950',
  error: 'bg-white text-red-700 hover:bg-red-100 hover:text-red-900',
} as const;

export function CopyButton({
  className,
  label,
  tone = 'default',
  value,
}: Readonly<{
  className?: string;
  label: string;
  tone?: keyof typeof toneClassNames;
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
        <button
          aria-label={copied ? 'Copied' : label}
          className={cn(
            'inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md transition-colors',
            toneClassNames[tone],
            className,
          )}
          onClick={(event) => {
            event.stopPropagation();
            void copyValue();
          }}
          type="button"
        >
          {copied ? (
            <Check aria-hidden="true" className="h-3.5 w-3.5 text-emerald-600" />
          ) : (
            <Copy aria-hidden="true" className="h-3.5 w-3.5" />
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent>{copied ? 'Copied' : label}</TooltipContent>
    </Tooltip>
  );
}
