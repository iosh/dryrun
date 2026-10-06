import { AlertTriangle, CheckCircle2, CircleSlash2, XCircle } from 'lucide-react';

import { cn } from '../../../lib/cn.ts';
import { formatAmount, formatHexQuantity, formatNativeAmount } from '../../../lib/formatting.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';
import { changesLabel } from '../../changes.ts';
import { getEnvironment } from '../../environment.ts';
import type { Outcome } from '../../rpc.ts';
import type { SimulationRecord } from '../../types.ts';
import { AddressValue } from './AddressHighlight.tsx';
import { DetailItem, SummaryMetric } from './ResultPrimitives.tsx';
import type { AddressHighlightController } from './useAddressHighlight.ts';

export function ExecutionSummary({ record }: Readonly<{ record: SimulationRecord }>) {
  const { outcome, transaction } = record.response;
  const environment = getEnvironment(record.environmentId);
  const anchor = 'epoch' in record.response ? record.response.epoch : record.response.block;
  const statusLabel = {
    success: 'Success', reverted: 'Reverted', halted: 'Halted', rejected: 'Rejected',
  }[outcome.status];
  const Icon = outcome.status === 'success' ? CheckCircle2
    : outcome.status === 'rejected' ? CircleSlash2 : XCircle;
  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <div className={cn(
        'flex flex-col gap-4 border-b px-5 py-5 sm:flex-row sm:items-center sm:justify-between',
        outcome.status === 'success' ? 'border-emerald-200 bg-emerald-50'
          : outcome.status === 'rejected' ? 'border-amber-200 bg-amber-50' : 'border-red-200 bg-red-50',
      )}>
        <div className="flex items-center gap-3">
          <Icon aria-hidden="true" className="h-6 w-6" />
          <div>
            <p className="text-xs font-medium text-ink-600">Execution</p>
            <h2 className="mt-1 text-xl font-semibold">{statusLabel}</h2>
          </div>
        </div>
        <div className="sm:text-right">
          <p className="text-sm font-medium">{environment.label} {environment.networkLabel}</p>
          <p className="mt-1 font-mono text-[11px] text-ink-600">
            {'epoch' in record.response ? 'Epoch' : 'Block'} {formatHexQuantity(anchor.number)}
          </p>
        </div>
      </div>
      <dl className="grid grid-cols-2 divide-x divide-line sm:grid-cols-4">
        <SummaryMetric label="Analysis" value={changesLabel(outcome)} />
        <SummaryMetric
          label="Gas used"
          value={outcome.status === 'rejected' ? '—' : formatHexQuantity(outcome.gasUsed)}
        />
        <SummaryMetric
          label="Fee"
          value={outcome.status === 'rejected' ? '—'
            : formatNativeAmount(outcome.fee.amount, environment.nativeSymbol)}
        />
        <SummaryMetric label="Chain ID" value={formatHexQuantity(transaction.chainId)} />
      </dl>
    </section>
  );
}

export function ExecutionFailure({ outcome }: Readonly<{ outcome: Outcome }>) {
  if (outcome.status === 'success') return null;
  return (
    <section className="border-l-2 border-red-500 bg-red-50 px-4 py-3 text-red-900">
      <div className="flex gap-3">
        <AlertTriangle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />
        <div className="min-w-0">
          <p className="break-words text-sm font-semibold">
            {outcome.status === 'rejected' ? outcome.message
              : outcome.reason ?? 'The transaction reverted without a decoded reason.'}
          </p>
          {outcome.status === 'rejected' ? <p className="mt-1 break-all font-mono text-[11px]">{outcome.reason}</p> : null}
        </div>
      </div>
    </section>
  );
}

export function ExecutionDetails({
  record,
  addressHighlight,
}: Readonly<{ record: SimulationRecord; addressHighlight: AddressHighlightController }>) {
  const { outcome } = record.response;
  const environment = getEnvironment(record.environmentId);
  const anchorHash = 'epoch' in record.response ? record.response.epoch.pivotHash : record.response.block.hash;
  const price = (value: string) => formatAmount(value, 9, environment.feeUnit);
  const payer = outcome.status === 'rejected' ? undefined : outcome.fee.payer;
  const payerAddress = payer?.type === 'sponsor' ? payer.contract
    : payer?.type === 'sender' ? payer.address : record.response.transaction.from;

  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <h3 className="border-b border-line px-5 py-4 text-base font-semibold">Execution details</h3>
      <dl className="grid sm:grid-cols-2 xl:grid-cols-3">
        <DetailItem label={'epoch' in record.response ? 'Pivot hash' : 'Block hash'} value={anchorHash} />
        {outcome.status !== 'rejected' ? (
          <>
            <DetailItem label="Gas price" value={price(outcome.fee.gasPrice)} />
            <DetailItem label="Base fee" value={price(outcome.fee.baseFee)} />
            {outcome.gasCharged !== undefined ? <DetailItem label="Gas charged" value={formatHexQuantity(outcome.gasCharged)} /> : null}
            {outcome.fee.blobGasPrice !== undefined ? <DetailItem label="Blob gas price" value={price(outcome.fee.blobGasPrice)} /> : null}
            <div className="min-w-0 border-b border-line px-5 py-4 sm:col-span-2 xl:col-span-3">
              <dt className="mb-1 text-[11px] text-ink-600">Fee payer · {payer?.type === 'sponsor' ? 'Contract sponsor pool' : 'Sender'}</dt>
              <dd><AddressValue address={payerAddress} addressHighlight={addressHighlight} /></dd>
            </div>
          </>
        ) : null}
      </dl>
      {'output' in outcome ? (
        <div className="relative bg-code px-5 py-4 pr-14">
          <p className="mb-2 text-[11px] text-code-ink">Output</p>
          <CopyButton className="absolute right-3 top-3" label="Copy output" tone="code" value={outcome.output} />
          <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px] leading-5 text-code-ink">{outcome.output}</pre>
        </div>
      ) : null}
    </section>
  );
}
