import { useMemo } from 'react';

import { cn } from '../../../lib/cn.ts';
import { AssetFlow } from './AssetFlow.tsx';
import { buildParticipantMap } from './resultModel.ts';
import { StateEffects } from './StateEffects.tsx';
import type {
  AddressHighlightController,
  SimulationResultViewModel,
} from './resultTypes.ts';

export function TransactionEffects({
  addressHighlight,
  viewModel,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  viewModel: SimulationResultViewModel;
}>) {
  const participantMap = useMemo(
    () => buildParticipantMap(viewModel.lanes),
    [viewModel.lanes],
  );

  return (
    <section className="overflow-hidden rounded-lg border border-line bg-white">
      <div className="flex items-center justify-between gap-4 border-b border-line px-5 py-4">
        <div>
          <p className="text-xs font-medium text-ink-600">Transaction</p>
          <h3 className="mt-1 text-lg font-semibold">Effects</h3>
        </div>
        <span className="flex h-7 min-w-7 items-center justify-center rounded-full bg-shell-100 px-2 text-xs font-semibold text-ink-600">
          {viewModel.changesError ? '—' : viewModel.changes.length}
        </span>
      </div>

      <div className="border-b border-line">
        <div className="px-5 py-3">
          <p className="text-xs font-semibold text-ink-950">Net outcome</p>
        </div>
        <div className="grid border-t border-line sm:grid-cols-2 xl:grid-cols-3">
          {viewModel.senderImpacts.map((impact, index) => (
            <div
              className="min-w-0 border-b border-line px-5 py-4 sm:border-r xl:[&:nth-child(3n)]:border-r-0"
              key={`${impact.label}:${impact.value}:${index}`}
            >
              <p
                className={cn(
                  'text-[11px] font-medium',
                  impact.tone === 'positive' && 'text-emerald-700',
                  impact.tone === 'negative' && 'text-red-700',
                  impact.tone === 'neutral' && 'text-ink-600',
                )}
              >
                {impact.label}
              </p>
              <p className="mt-1 min-w-0 break-words text-sm font-semibold text-ink-950">
                {impact.value}
              </p>
            </div>
          ))}
        </div>
      </div>

      <div className="flex items-center justify-between gap-4 border-b border-line px-5 py-3">
        <p className="text-xs font-semibold text-ink-950">Asset flow</p>
        <span className="text-[11px] text-ink-400">
          {viewModel.lanes.length}{' '}
          {viewModel.lanes.length === 1 ? 'lane' : 'lanes'}
        </span>
      </div>
      <AssetFlow
        addressHighlight={addressHighlight}
        flowItems={viewModel.flowItems}
        lanes={viewModel.lanes}
      />

      <StateEffects
        addressHighlight={addressHighlight}
        changes={viewModel.stateEffects}
        participantMap={participantMap}
      />
    </section>
  );
}
