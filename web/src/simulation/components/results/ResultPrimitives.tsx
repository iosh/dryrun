import { ChevronDown, FileJson2 } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../../../lib/cn.ts';
import { formatJson } from '../../../lib/formatting.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';

type ChangeTone = 'amber' | 'blue' | 'green' | 'red' | 'violet';

export function ResultShell({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <section className="rounded-lg border border-line bg-white p-5 sm:p-6">
      {children}
    </section>
  );
}

export function SummaryMetric({
  label,
  value,
}: Readonly<{ label: string; value: string }>) {
  return (
    <div className="min-w-0 px-4 py-4 even:border-l even:border-line sm:even:border-l-0">
      <dt className="text-[11px] font-medium text-ink-600">{label}</dt>
      <dd
        className="mt-1 break-all text-sm font-semibold text-ink-950"
        title={value}
      >
        {value}
      </dd>
    </div>
  );
}

export function DetailItem({
  label,
  value,
}: Readonly<{ label: string; value: string }>) {
  return (
    <div className="min-w-0 border-b border-line px-5 py-4 sm:border-r xl:[&:nth-child(3n)]:border-r-0">
      <dt className="text-[11px] font-medium text-ink-600">{label}</dt>
      <dd
        className="mt-1 truncate text-sm font-medium text-ink-950"
        title={value}
      >
        {value}
      </dd>
    </div>
  );
}

export function ChangeBadge({
  label,
  tone,
}: Readonly<{ label: string; tone: ChangeTone }>) {
  const toneClassName: Record<ChangeTone, string> = {
    amber: 'bg-amber-100 text-amber-800',
    blue: 'bg-blue-100 text-blue-800',
    green: 'bg-emerald-100 text-emerald-800',
    red: 'bg-red-100 text-red-800',
    violet: 'bg-violet-100 text-violet-800',
  };

  return (
    <span
      className={cn(
        'max-w-full break-all rounded px-2 py-1 text-[10px] font-semibold',
        toneClassName[tone],
      )}
    >
      {label}
    </span>
  );
}

export function RawJsonDetails({
  label,
  value,
}: Readonly<{ label: string; value: unknown }>) {
  const formattedValue = formatJson(value);

  return (
    <details className="group overflow-hidden rounded-lg border border-line bg-white">
      <summary className="flex min-h-13 cursor-pointer list-none items-center justify-between gap-4 px-5 text-sm font-medium [&::-webkit-details-marker]:hidden">
        <span className="flex items-center gap-2">
          <FileJson2 aria-hidden="true" className="h-4 w-4 text-ink-600" />
          {label}
        </span>
        <ChevronDown
          aria-hidden="true"
          className="h-4 w-4 text-ink-600 transition-transform group-open:rotate-180"
        />
      </summary>
      <div className="relative border-t border-line bg-code">
        <CopyButton
          className="absolute right-3 top-3 z-10"
          tone="code"
          label={`Copy ${label.toLowerCase()}`}
          value={formattedValue}
        />
        <pre className="max-h-130 overflow-auto px-5 py-4 pr-14 font-mono text-[11px] leading-5 text-code-ink">
          {formattedValue}
        </pre>
      </div>
    </details>
  );
}

export function ChangeSection({
  title,
  count,
  description,
  children,
}: Readonly<{ title: string; count: number; description?: string; children: ReactNode }>) {
  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <div className="border-b border-line px-5 py-4">
        <h3 className="text-base font-semibold">{title} <span className="ml-1 text-xs font-normal text-ink-400">{count}</span></h3>
        {description ? <p className="mt-1 text-xs leading-5 text-ink-600">{description}</p> : null}
      </div>
      <div className="divide-y divide-line">{children}</div>
    </section>
  );
}

export function StateDifference({
  before,
  after,
}: Readonly<{ before: ReactNode; after: ReactNode }>) {
  return (
    <dl className="mt-3 grid gap-3 rounded-md bg-shell-50 p-3 sm:grid-cols-2">
      <div className="min-w-0">
        <dt className="mb-1 text-[11px] text-ink-400">Before</dt>
        <dd className="break-words text-xs leading-6 text-ink-600">{before}</dd>
      </div>
      <div className="min-w-0">
        <dt className="mb-1 text-[11px] text-ink-400">After</dt>
        <dd className="break-words text-xs leading-6 text-ink-950">{after}</dd>
      </div>
    </dl>
  );
}
