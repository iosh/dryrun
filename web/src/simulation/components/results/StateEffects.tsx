import { cn } from '../../../lib/cn.ts';
import { toChangeItemViewModel } from '../../changeView.ts';
import {
  getChangeAddresses,
  normalizeAddress,
} from '../../flowView.ts';
import type {
  SimulationChange,
  SimulationRecord,
} from '../../types.ts';
import {
  AddressAliasValue,
  AddressValue,
} from './AddressHighlight.tsx';
import { hasHighlightedAddress } from './resultModel.ts';
import { ChangeBadge } from './ResultPrimitives.tsx';
import type {
  AddressHighlightController,
  FlowLaneViewModel,
} from './resultTypes.ts';

export function StateEffects({
  addressHighlight,
  changes,
  environmentId,
  participantMap,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  changes: readonly SimulationChange[];
  environmentId: SimulationRecord['environmentId'];
  participantMap: ReadonlyMap<string, FlowLaneViewModel>;
}>) {
  if (changes.length === 0) return null;

  return (
    <>
      <div className="border-y border-line px-5 py-3">
        <p className="text-xs font-semibold text-ink-950">State effects</p>
      </div>
      <div>
        {changes.map((change, index) => (
          <StateEffectSummary
            addressHighlight={addressHighlight}
            change={change}
            environmentId={environmentId}
            key={`${change.changeType}:${index}`}
            participantMap={participantMap}
          />
        ))}
      </div>
    </>
  );
}

function StateEffectSummary({
  addressHighlight,
  change,
  environmentId,
  participantMap,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  change: SimulationChange;
  environmentId: SimulationRecord['environmentId'];
  participantMap: ReadonlyMap<string, FlowLaneViewModel>;
}>) {
  const view = toChangeItemViewModel(change, environmentId);
  const addresses = getChangeAddresses(change);
  const highlighted = hasHighlightedAddress(
    addresses,
    addressHighlight.activeAddress,
  );
  const actors = addresses.filter(
    (address) => address.label !== 'Asset contract',
  );

  return (
    <article
      className={cn(
        'border-b border-line px-5 py-4 transition-colors last:border-b-0',
        highlighted && 'bg-amber-50',
      )}
    >
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <ChangeBadge label={view.label} tone={view.tone} />
            <p className="text-sm font-semibold text-ink-950">{view.title}</p>
          </div>
          {view.detail ? (
            <p className="mt-2 text-sm text-ink-600">{view.detail}</p>
          ) : null}
        </div>
        {view.value ? (
          <p className="max-w-full break-words text-sm font-semibold sm:max-w-[48%] sm:text-right">
            {view.value}
          </p>
        ) : null}
      </div>
      {actors.length > 0 ? (
        <div className="mt-3 space-y-2">
          {actors.map((actor, index) => {
            const participant = participantMap.get(
              normalizeAddress(actor.address),
            );
            return (
              <div
                className="grid min-w-0 gap-1 sm:grid-cols-[96px_minmax(0,1fr)]"
                key={`${actor.label}:${actor.address}:${index}`}
              >
                <span className="text-[11px] text-ink-400">{actor.label}</span>
                <div className="flex min-w-0 flex-wrap items-start gap-2">
                  <AddressAliasValue
                    address={actor.address}
                    addressHighlight={addressHighlight}
                    label={participant?.alias ?? actor.label}
                  />
                  <AddressValue
                    address={actor.address}
                    addressHighlight={addressHighlight}
                  />
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
    </article>
  );
}
