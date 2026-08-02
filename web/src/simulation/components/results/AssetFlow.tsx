import { ArrowLeft, ArrowRight } from 'lucide-react';

import { cn } from '../../../lib/cn.ts';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '../../../ui/Tooltip.tsx';
import type { ChangeTone } from '../../changeView.ts';
import {
  normalizeAddress,
  type FlowEndpoint,
} from '../../flowView.ts';
import { AddressValue } from './AddressHighlight.tsx';
import {
  flowEndpointLaneKey,
  flowItemAddresses,
  getFlowSegment,
  hasHighlightedAddress,
} from './resultModel.ts';
import { ChangeBadge } from './ResultPrimitives.tsx';
import type {
  AddressHighlightController,
  FlowLaneViewModel,
  FlowSegment,
  SequencedAssetFlowItemViewModel,
} from './resultTypes.ts';

export function AssetFlow({
  addressHighlight,
  flowItems,
  lanes,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  flowItems: readonly SequencedAssetFlowItemViewModel[];
  lanes: readonly FlowLaneViewModel[];
}>) {
  if (flowItems.length === 0) {
    return (
      <div className="px-5 py-8 text-center text-sm text-ink-600">
        No asset movement
      </div>
    );
  }

  const laneIndex = new Map(
    lanes.map((lane, index) => [lane.key, index] as const),
  );
  const gridTemplateColumns = `repeat(${lanes.length}, minmax(220px, 1fr))`;

  return (
    <div className="overflow-x-auto">
      <div style={{ minWidth: `${Math.max(440, lanes.length * 220)}px` }}>
        <div
          className="grid border-b border-line bg-shell-50"
          style={{ gridTemplateColumns }}
        >
          {lanes.map((lane) => (
            <FlowLaneHeader
              addressHighlight={addressHighlight}
              key={lane.key}
              lane={lane}
            />
          ))}
        </div>

        <div>
          {flowItems.map((item) => (
            <FlowMovementRow
              addressHighlight={addressHighlight}
              gridTemplateColumns={gridTemplateColumns}
              item={item}
              key={`${item.changeIndex}:${item.assetKey}`}
              laneIndex={laneIndex}
              lanes={lanes}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function FlowLaneHeader({
  addressHighlight,
  lane,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  lane: FlowLaneViewModel;
}>) {
  const active =
    lane.kind === 'address' &&
    addressHighlight.activeAddress === normalizeAddress(lane.address);

  return (
    <div
      className={cn(
        'min-w-0 border-r border-line px-4 py-4 transition-colors last:border-r-0',
        active && 'bg-amber-50',
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs font-semibold text-ink-950">{lane.alias}</p>
        {lane.kind === 'address' && lane.context ? (
          <span className="text-[10px] font-medium text-ink-400">
            {lane.context}
          </span>
        ) : null}
      </div>
      {lane.kind === 'address' ? (
        <div className="mt-2">
          <AddressValue
            address={lane.address}
            addressHighlight={addressHighlight}
          />
        </div>
      ) : (
        <p className="mt-2 text-[11px] text-ink-400">Flow boundary</p>
      )}
    </div>
  );
}

function FlowMovementRow({
  addressHighlight,
  gridTemplateColumns,
  item,
  laneIndex,
  lanes,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  gridTemplateColumns: string;
  item: SequencedAssetFlowItemViewModel;
  laneIndex: ReadonlyMap<string, number>;
  lanes: readonly FlowLaneViewModel[];
}>) {
  const fromIndex = laneIndex.get(flowEndpointLaneKey(item.from))!;
  const toIndex = laneIndex.get(flowEndpointLaneKey(item.to))!;
  const lowIndex = Math.min(fromIndex, toIndex);
  const highIndex = Math.max(fromIndex, toIndex);
  const direction = Math.sign(toIndex - fromIndex);
  const highlighted = hasHighlightedAddress(
    flowItemAddresses(item),
    addressHighlight.activeAddress,
  );

  return (
    <div
      aria-label={`${item.label}: ${item.value}`}
      className={cn(
        'relative border-b border-line transition-colors last:border-b-0',
        highlighted && 'bg-amber-50',
      )}
    >
      <div
        className="pointer-events-none absolute inset-x-0 top-3 z-20 grid"
        style={{ gridTemplateColumns }}
      >
        <div
          className="min-w-0 px-3 text-center"
          style={{ gridColumn: `${lowIndex + 1} / ${highIndex + 2}` }}
        >
          <div className="inline-flex max-w-full items-center gap-2 bg-white px-2">
            <ChangeBadge label={item.label} tone={item.tone} />
            <span className="min-w-0 break-words text-xs font-semibold text-ink-950">
              {item.value}
            </span>
          </div>
          <p className="mt-1 text-[10px] text-ink-400">
            Change {item.changeIndex + 1}
          </p>
        </div>
      </div>

      <div className="grid min-h-28" style={{ gridTemplateColumns }}>
        {lanes.map((lane, index) => {
          const isFrom = index === fromIndex;
          const isTo = index === toIndex;
          const segment = getFlowSegment(index, fromIndex, toIndex);

          return (
            <div
              className="relative min-h-28 border-r border-line/70 last:border-r-0"
              key={lane.key}
            >
              <span className="absolute inset-y-0 left-1/2 w-px bg-line/80" />
              {segment ? (
                <span
                  className={cn(
                    'absolute top-19 h-px',
                    flowSegmentClassName(segment),
                    flowLineClassName(item.tone),
                  )}
                />
              ) : null}
              {isFrom ? (
                <FlowEndpointMarker
                  addressHighlight={addressHighlight}
                  direction={direction}
                  endpoint={item.from}
                  isDestination={isTo}
                  lane={lane}
                  tone={item.tone}
                />
              ) : null}
              {isTo && !isFrom ? (
                <FlowEndpointMarker
                  addressHighlight={addressHighlight}
                  direction={direction}
                  endpoint={item.to}
                  isDestination
                  lane={lane}
                  tone={item.tone}
                />
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function FlowEndpointMarker({
  addressHighlight,
  direction,
  endpoint,
  isDestination,
  lane,
  tone,
}: Readonly<{
  addressHighlight: AddressHighlightController;
  direction: number;
  endpoint: FlowEndpoint;
  isDestination: boolean;
  lane: FlowLaneViewModel;
  tone: ChangeTone;
}>) {
  const marker = isDestination && direction !== 0
    ? direction > 0
      ? <ArrowRight aria-hidden="true" className="h-3 w-3" />
      : <ArrowLeft aria-hidden="true" className="h-3 w-3" />
    : <span className="h-1.5 w-1.5 rounded-full bg-current" />;
  const className = cn(
    'absolute left-1/2 top-16.5 z-10 flex h-5 w-5 -translate-x-1/2 items-center justify-center rounded-full border bg-white',
    flowMarkerClassName(tone),
  );

  if (endpoint.kind === 'terminal') {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <span aria-label={endpoint.label} className={className}>
            {marker}
          </span>
        </TooltipTrigger>
        <TooltipContent>{endpoint.label}</TooltipContent>
      </Tooltip>
    );
  }

  const normalized = normalizeAddress(endpoint.address);
  const active = addressHighlight.activeAddress === normalized;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          aria-label={`Highlight ${lane.alias}: ${endpoint.address}`}
          aria-pressed={addressHighlight.pinnedAddress === normalized}
          className={cn(className, active && 'ring-2 ring-amber-300')}
          data-address-value=""
          onBlur={addressHighlight.onAddressLeave}
          onClick={() => addressHighlight.onAddressToggle(normalized)}
          onFocus={() => addressHighlight.onAddressEnter(normalized)}
          onMouseEnter={() => addressHighlight.onAddressEnter(normalized)}
          onMouseLeave={addressHighlight.onAddressLeave}
          type="button"
        >
          {marker}
        </button>
      </TooltipTrigger>
      <TooltipContent className="font-mono">
        {lane.alias}: {endpoint.address}
      </TooltipContent>
    </Tooltip>
  );
}

function flowSegmentClassName(segment: Exclude<FlowSegment, null>) {
  switch (segment) {
    case 'full':
      return 'inset-x-0';
    case 'left-half':
      return 'left-0 right-1/2';
    case 'right-half':
      return 'left-1/2 right-0';
  }
}

function flowLineClassName(tone: ChangeTone) {
  const classNames: Record<ChangeTone, string> = {
    amber: 'bg-amber-400',
    blue: 'bg-blue-400',
    green: 'bg-emerald-400',
    red: 'bg-red-400',
    violet: 'bg-violet-400',
  };
  return classNames[tone];
}

function flowMarkerClassName(tone: ChangeTone) {
  const classNames: Record<ChangeTone, string> = {
    amber: 'border-amber-400 text-amber-700',
    blue: 'border-blue-400 text-blue-700',
    green: 'border-emerald-400 text-emerald-700',
    red: 'border-red-400 text-red-700',
    violet: 'border-violet-400 text-violet-700',
  };
  return classNames[tone];
}
