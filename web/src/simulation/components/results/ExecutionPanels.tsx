import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  CircleSlash2,
  Code2,
  XCircle,
} from 'lucide-react';

import { cn } from '../../../lib/cn.ts';
import {
  formatHexQuantity,
  formatNativeAmount,
} from '../../../lib/formatting.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';
import type {
  ExecutionAnchor,
  SimulationExecution,
} from './resultTypes.ts';
import { DetailItem, SummaryMetric } from './ResultPrimitives.tsx';

export function ExecutionSummary({
  anchor,
  changesCount,
  execution,
  nativeSymbol,
  networkLabel,
}: Readonly<{
  anchor: ExecutionAnchor;
  changesCount: string;
  execution: SimulationExecution;
  nativeSymbol: string;
  networkLabel: string;
}>) {
  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <div
        className={cn(
          'flex flex-col gap-4 border-b px-5 py-5 sm:flex-row sm:items-center sm:justify-between',
          execution.status === 'SUCCESS'
            ? 'border-emerald-200 bg-emerald-50'
            : execution.status === 'FAILED'
              ? 'border-red-200 bg-red-50'
              : 'border-amber-200 bg-amber-50',
        )}
      >
        <div className="flex items-center gap-3">
          <StatusIcon status={execution.status} />
          <div>
            <p className="text-xs font-medium text-ink-600">Execution</p>
            <h2 className="mt-1 text-xl font-semibold">
              {statusLabel(execution.status)}
            </h2>
          </div>
        </div>
        <div className="sm:text-right">
          <p className="text-sm font-medium">{networkLabel}</p>
          <p className="mt-1 font-mono text-[11px] text-ink-600">
            {anchor.label} {formatHexQuantity(anchor.number)}
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 divide-x divide-line sm:grid-cols-4">
        <SummaryMetric label="Changes" value={String(changesCount)} />
        <SummaryMetric
          label="Gas used"
          value={formatHexQuantity(execution.gasUsed)}
        />
        <SummaryMetric
          label="Fee"
          value={formatNativeAmount(execution.fee, nativeSymbol)}
        />
        <SummaryMetric
          label="Chain ID"
          value={formatHexQuantity(execution.chainId)}
        />
      </div>
    </section>
  );
}

export function ExecutionFailure({
  failure,
}: Readonly<{
  failure: { code: string; message: string; reason?: string | null };
}>) {
  return (
    <section className="border-l-2 border-red-500 bg-red-50 px-4 py-3 text-red-900">
      <div className="flex gap-3">
        <AlertTriangle
          aria-hidden="true"
          className="mt-0.5 h-4 w-4 shrink-0"
        />
        <div className="min-w-0">
          <p className="text-sm font-semibold">{failure.message}</p>
          <p className="mt-1 font-mono text-[11px] text-red-800">
            {failure.code}
            {failure.reason ? ` / ${failure.reason}` : ''}
          </p>
        </div>
      </div>
    </section>
  );
}

export function ExecutionDetails({
  anchor,
  execution,
  nativeSymbol,
}: Readonly<{
  anchor: ExecutionAnchor;
  execution: SimulationExecution;
  nativeSymbol: string;
}>) {
  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <div className="border-b border-line px-5 py-4">
        <p className="text-xs font-medium text-ink-600">Execution</p>
        <h3 className="mt-1 text-lg font-semibold">Details</h3>
      </div>

      <dl className="grid sm:grid-cols-2 xl:grid-cols-3">
        <DetailItem
          label="Gas used"
          value={formatHexQuantity(execution.gasUsed)}
        />
        <DetailItem
          label="Gas limit"
          value={formatHexQuantity(execution.gasLimit)}
        />
        {'gasCharged' in execution ? (
          <DetailItem
            label="Gas charged"
            value={formatHexQuantity(execution.gasCharged)}
          />
        ) : null}
        <DetailItem
          label="Fee"
          value={formatNativeAmount(execution.fee, nativeSymbol)}
        />
        <DetailItem
          label="Burnt fee"
          value={
            execution.burntFee
              ? formatNativeAmount(execution.burntFee, nativeSymbol)
              : 'None'
          }
        />
        <DetailItem
          label={anchor.label}
          value={formatHexQuantity(anchor.number)}
        />
        {'gasCoveredBySponsor' in execution ? (
          <>
            <DetailItem
              label="Gas sponsored"
              value={execution.gasCoveredBySponsor ? 'Yes' : 'No'}
            />
            <DetailItem
              label="Storage sponsored"
              value={execution.storageCoveredBySponsor ? 'Yes' : 'No'}
            />
          </>
        ) : null}
      </dl>

      <div className="border-t border-line px-5 py-4">
        <p className="text-xs font-medium text-ink-600">
          {anchor.label} hash
        </p>
        <div className="mt-2 flex min-w-0 items-start gap-2">
          <p className="min-w-0 flex-1 break-all font-mono text-[11px] leading-5 text-ink-950">
            {anchor.hash}
          </p>
          <CopyButton label={`Copy ${anchor.label.toLowerCase()} hash`} value={anchor.hash} />
        </div>
      </div>

      <details className="group border-t border-line">
        <summary className="flex min-h-12 cursor-pointer list-none items-center justify-between gap-4 px-5 text-sm font-medium [&::-webkit-details-marker]:hidden">
          <span className="flex items-center gap-2">
            <Code2 aria-hidden="true" className="h-4 w-4 text-ink-600" />
            Output
          </span>
          <ChevronDown
            aria-hidden="true"
            className="h-4 w-4 text-ink-600 transition-transform group-open:rotate-180"
          />
        </summary>
        <div className="relative border-t border-line bg-code">
          <CopyButton
            className="absolute right-3 top-3 z-10 bg-code text-code-ink hover:bg-white/10 hover:text-white"
            label="Copy output"
            value={execution.output}
          />
          <pre className="max-h-72 overflow-auto px-5 py-4 pr-14 font-mono text-[11px] leading-5 text-code-ink">
            {execution.output}
          </pre>
        </div>
      </details>
    </section>
  );
}

function StatusIcon({
  status,
}: Readonly<{ status: SimulationExecution['status'] }>) {
  if (status === 'SUCCESS') {
    return (
      <CheckCircle2 aria-hidden="true" className="h-7 w-7 text-emerald-600" />
    );
  }
  if (status === 'FAILED') {
    return <XCircle aria-hidden="true" className="h-7 w-7 text-red-600" />;
  }
  return (
    <CircleSlash2 aria-hidden="true" className="h-7 w-7 text-amber-600" />
  );
}

function statusLabel(status: SimulationExecution['status']) {
  if (status === 'SUCCESS') return 'Success';
  if (status === 'FAILED') return 'Failed';
  return 'Not executed';
}
