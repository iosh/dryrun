import { ChevronDown, Database } from 'lucide-react';

import { cn } from '../../../lib/cn.ts';
import { formatJson } from '../../../lib/formatting.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';
import { toChangeItemViewModel } from '../../changeView.ts';
import {
  getChangeAddresses,
  normalizeAddress,
  type ChangeAddressViewModel,
} from '../../flowView.ts';
import type {
  SimulationChange,
  SimulationRecord,
} from '../../types.ts';
import { AddressValue } from './AddressHighlight.tsx';
import {
  changeAddressLabel,
  hasHighlightedAddress,
} from './resultModel.ts';
import { ChangeBadge } from './ResultPrimitives.tsx';
import type { AddressHighlightController } from './resultTypes.ts';

export function ChangesList({
  addressHighlight,
  changes,
  record,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  changes: readonly SimulationChange[];
  record: SimulationRecord;
}>) {
  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <div className="flex items-center justify-between gap-4 border-b border-line px-5 py-4">
        <div>
          <p className="text-xs font-medium text-ink-600">Backend output</p>
          <h3 className="mt-1 text-lg font-semibold">Changes</h3>
        </div>
        <span className="flex h-7 min-w-7 items-center justify-center rounded-full bg-shell-100 px-2 text-xs font-semibold text-ink-600">
          {changes.length}
        </span>
      </div>

      {changes.length > 0 ? (
        <div>
          {changes.map((change, index) => (
            <ChangeRow
              addressHighlight={addressHighlight}
              change={change}
              environmentId={record.environmentId}
              key={`${record.id}:${index}`}
              record={record}
            />
          ))}
        </div>
      ) : (
        <div className="px-5 py-10 text-center text-sm text-ink-600">
          No changes
        </div>
      )}
    </section>
  );
}

function ChangeRow({
  addressHighlight,
  change,
  environmentId,
  record,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  change: SimulationChange;
  environmentId: SimulationRecord['environmentId'];
  record: SimulationRecord;
}>) {
  const view = toChangeItemViewModel(change, environmentId);
  const addresses = getChangeAddresses(change);
  const highlighted = hasHighlightedAddress(
    addresses,
    addressHighlight.activeAddress,
  );
  const identifierIsAddress =
    view.identifier !== undefined &&
    addresses.some(
      (item) =>
        normalizeAddress(item.address) === normalizeAddress(view.identifier!),
    );
  const rawChange = formatJson(change);

  return (
    <article
      className={cn(
        'border-b border-line transition-colors last:border-b-0',
        highlighted && 'bg-amber-50',
      )}
    >
      <div className="px-5 py-4">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <ChangeBadge label={view.label} tone={view.tone} />
              <h4 className="text-sm font-semibold text-ink-950">
                {view.title}
              </h4>
            </div>
            {view.detail ? (
              <p className="mt-2 text-sm leading-5 text-ink-600">
                {view.detail}
              </p>
            ) : null}
            <ChangeAddressList
              addressHighlight={addressHighlight}
              addresses={addresses}
              record={record}
            />
            {view.identifier && !identifierIsAddress ? (
              <p
                className="mt-2 flex min-w-0 items-center gap-1.5 break-all font-mono text-[11px] leading-5 text-ink-400"
                title={view.identifier}
              >
                <Database aria-hidden="true" className="h-3.5 w-3.5 shrink-0" />
                {view.identifier}
              </p>
            ) : null}
          </div>
          {view.value ? (
            <p className="max-w-full break-words text-sm font-semibold text-ink-950 sm:max-w-[48%] sm:text-right">
              {view.value}
            </p>
          ) : null}
        </div>
      </div>
      <details className="group border-t border-line/70 bg-shell-50">
        <summary className="flex min-h-10 cursor-pointer list-none items-center justify-between gap-4 px-5 text-xs font-medium text-ink-600 [&::-webkit-details-marker]:hidden">
          <span>Raw change</span>
          <ChevronDown
            aria-hidden="true"
            className="h-3.5 w-3.5 transition-transform group-open:rotate-180"
          />
        </summary>
        <div className="relative border-t border-line bg-code">
          <CopyButton
            className="absolute right-3 top-3 z-10"
            label="Copy raw change"
            tone="code"
            value={rawChange}
          />
          <pre className="max-h-80 overflow-auto px-5 py-4 pr-14 font-mono text-[11px] leading-5 text-code-ink">
            {rawChange}
          </pre>
        </div>
      </details>
    </article>
  );
}

function ChangeAddressList({
  addressHighlight,
  addresses,
  record,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  addresses: readonly ChangeAddressViewModel[];
  record: SimulationRecord;
}>) {
  if (addresses.length === 0) return null;

  return (
    <div className="mt-3 space-y-2">
      {addresses.map((item, index) => (
        <div
          className="grid min-w-0 gap-1 sm:grid-cols-[96px_minmax(0,1fr)]"
          key={`${item.label}:${item.address}:${index}`}
        >
          <span className="text-[11px] font-medium text-ink-400">
            {changeAddressLabel(item, record)}
          </span>
          <AddressValue
            address={item.address}
            addressHighlight={addressHighlight}
          />
        </div>
      ))}
    </div>
  );
}
