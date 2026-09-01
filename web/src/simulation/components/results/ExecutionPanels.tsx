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
          execution.status === 'success'
            ? 'border-emerald-200 bg-emerald-50'
            : execution.status === 'rejected'
              ? 'border-amber-200 bg-amber-50'
              : 'border-red-200 bg-red-50',
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
          value={execution.gasUsed ? formatHexQuantity(execution.gasUsed) : '—'}
        />
        <SummaryMetric
          label="Fee"
          value={
            execution.totalFee
              ? formatNativeAmount(execution.totalFee, nativeSymbol)
              : '—'
          }
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
  failure: { detail?: string; message: string };
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
          {failure.detail ? (
            <p className="mt-1 font-mono text-[11px] text-red-800">
              {failure.detail}
            </p>
          ) : null}
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
          value={execution.gasUsed ? formatHexQuantity(execution.gasUsed) : 'Not executed'}
        />
        <DetailItem
          label="Gas limit"
          value={formatHexQuantity(execution.gasLimit)}
        />
        {execution.effectiveGasPrice ? (
          <DetailItem
            label="Effective gas price"
            value={formatHexQuantity(execution.effectiveGasPrice)}
          />
        ) : null}
        {execution.gasCharged ? (
          <DetailItem
            label="Gas charged"
            value={formatHexQuantity(execution.gasCharged)}
          />
        ) : null}
        {execution.gasFee ? (
          <DetailItem
            label="Gas fee"
            value={formatNativeAmount(execution.gasFee, nativeSymbol)}
          />
        ) : null}
        {execution.blobGasUsed ? (
          <DetailItem
            label="Blob gas used"
            value={formatHexQuantity(execution.blobGasUsed)}
          />
        ) : null}
        {execution.blobGasPrice ? (
          <DetailItem
            label="Blob gas price"
            value={formatHexQuantity(execution.blobGasPrice)}
          />
        ) : null}
        {execution.blobGasFee ? (
          <DetailItem
            label="Blob gas fee"
            value={formatNativeAmount(execution.blobGasFee, nativeSymbol)}
          />
        ) : null}
        {execution.totalFee ? (
          <DetailItem
            label="Total fee"
            value={formatNativeAmount(execution.totalFee, nativeSymbol)}
          />
        ) : null}
        {execution.burntGasFee !== null ? (
          <DetailItem
            label="Burnt gas fee"
            value={formatNativeAmount(execution.burntGasFee, nativeSymbol)}
          />
        ) : null}
        <DetailItem label="Logs" value={String(execution.logsCount)} />
        <DetailItem
          label={anchor.label}
          value={formatHexQuantity(anchor.number)}
        />
        {execution.gasCoveredBySponsor !== null ? (
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

      {execution.contractAddress ? (
        <div className="border-t border-line px-5 py-4">
          <p className="text-xs font-medium text-ink-600">Contract address</p>
          <p className="mt-2 break-all font-mono text-[11px] leading-5 text-ink-950">
            {execution.contractAddress}
          </p>
        </div>
      ) : null}

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

      {execution.output ? <details className="group border-t border-line">
        <summary className="flex min-h-12 cursor-pointer list-none items-center justify-between gap-4 px-5 text-sm font-medium [&::-webkit-details-marker]:hidden">
          <span className="flex items-center gap-2">
            <Code2 aria-hidden="true" className="h-4 w-4 text-ink-600" />
            {execution.output.label}
          </span>
          <ChevronDown
            aria-hidden="true"
            className="h-4 w-4 text-ink-600 transition-transform group-open:rotate-180"
          />
        </summary>
        <div className="relative border-t border-line bg-code">
          <CopyButton
            className="absolute right-3 top-3 z-10 bg-code text-code-ink hover:bg-white/10 hover:text-white"
            label={`Copy ${execution.output.label.toLowerCase()}`}
            value={execution.output.value}
          />
          <pre className="max-h-72 overflow-auto px-5 py-4 pr-14 font-mono text-[11px] leading-5 text-code-ink">
            {execution.output.value}
          </pre>
        </div>
      </details> : null}
    </section>
  );
}

function StatusIcon({
  status,
}: Readonly<{ status: SimulationExecution['status'] }>) {
  if (status === 'success') {
    return (
      <CheckCircle2 aria-hidden="true" className="h-7 w-7 text-emerald-600" />
    );
  }
  if (status === 'failed' || status === 'reverted') {
    return <XCircle aria-hidden="true" className="h-7 w-7 text-red-600" />;
  }
  return (
    <CircleSlash2 aria-hidden="true" className="h-7 w-7 text-amber-600" />
  );
}

function statusLabel(status: SimulationExecution['status']) {
  if (status === 'success') return 'Success';
  if (status === 'reverted') return 'Reverted';
  if (status === 'failed') return 'Failed';
  return 'Rejected';
}
