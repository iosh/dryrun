import { History, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';

import { cn } from '../../lib/cn.ts';
import {
  formatTimestampLabel,
  formatJson,
  shortHex,
} from '../../lib/formatting.ts';
import { Button } from '../../ui/Button.tsx';
import { CopyButton } from '../../ui/CopyButton.tsx';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetTitle,
  SheetTrigger,
} from '../../ui/Sheet.tsx';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '../../ui/Tooltip.tsx';
import { getEnvironment } from '../environment.ts';
import { HISTORY_LIMIT } from '../history.ts';
import {
  simulationChanges,
  type SimulationRecord,
} from '../types.ts';

export interface SimulationHistoryProps {
  activeRecordId: string | null;
  history: readonly SimulationRecord[];
  isBusy: boolean;
  onDeleteHistoryEntry: (id: string) => void;
  onNewSimulation: () => void;
  onSelectHistoryEntry: (id: string) => void;
}

export function SimulationHistorySidebar(
  props: Readonly<SimulationHistoryProps>,
) {
  return (
    <aside className="hidden min-h-0 border-r border-line bg-shell-100 lg:flex lg:flex-col">
      <HistoryHeading {...props} />
      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-4">
        <HistoryList {...props} />
      </div>
    </aside>
  );
}

export function SimulationHistoryMobile(
  props: Readonly<SimulationHistoryProps>,
) {
  const [open, setOpen] = useState(false);

  function startNewSimulation() {
    props.onNewSimulation();
    setOpen(false);
  }

  function selectHistoryEntry(id: string) {
    props.onSelectHistoryEntry(id);
    setOpen(false);
  }

  return (
    <Sheet onOpenChange={setOpen} open={open}>
      <SheetTrigger asChild>
        <Button
          aria-label={`Open history, ${props.history.length} records`}
          className="gap-2"
          size="sm"
          variant="secondary"
        >
          <History aria-hidden="true" className="h-4 w-4 text-ink-600" />
          <span className="text-xs">{props.history.length}</span>
        </Button>
      </SheetTrigger>
      <SheetContent>
        <div className="border-b border-line px-5 pb-4 pt-5 pr-14">
          <SheetTitle className="text-base font-semibold">History</SheetTitle>
          <SheetDescription className="mt-1 text-xs text-ink-600">
            {props.history.length} of {HISTORY_LIMIT} simulations
          </SheetDescription>
          <Button
            className="mt-4 w-full gap-2"
            disabled={props.isBusy}
            onClick={startNewSimulation}
            size="md"
          >
            <Plus aria-hidden="true" className="h-4 w-4" />
            New simulation
          </Button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <HistoryList
            {...props}
            onNewSimulation={startNewSimulation}
            onSelectHistoryEntry={selectHistoryEntry}
          />
        </div>
      </SheetContent>
    </Sheet>
  );
}

function HistoryHeading({
  history,
  isBusy,
  onNewSimulation,
}: Readonly<SimulationHistoryProps>) {
  return (
    <div className="p-4">
      <div className="mb-4 flex items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold">History</h2>
          <p className="mt-1 text-xs text-ink-600">
            {history.length} of {HISTORY_LIMIT}
          </p>
        </div>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label="New simulation"
              disabled={isBusy}
              onClick={onNewSimulation}
              size="icon"
            >
              <Plus aria-hidden="true" className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>New simulation</TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}

function HistoryList({
  activeRecordId,
  history,
  isBusy,
  onDeleteHistoryEntry,
  onSelectHistoryEntry,
}: Readonly<SimulationHistoryProps>) {
  if (history.length === 0) {
    return (
      <div className="flex min-h-36 flex-col items-center justify-center border border-dashed border-line bg-white px-4 text-center">
        <History aria-hidden="true" className="h-5 w-5 text-ink-400" />
        <p className="mt-3 text-sm font-medium text-ink-600">No history</p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border border-line bg-white">
      {history.map((record) => (
        <HistoryEntry
          disabled={isBusy}
          key={record.id}
          onDeleteHistoryEntry={onDeleteHistoryEntry}
          onSelectHistoryEntry={onSelectHistoryEntry}
          record={record}
          selected={record.id === activeRecordId}
        />
      ))}
    </div>
  );
}

function HistoryEntry({
  disabled,
  onDeleteHistoryEntry,
  onSelectHistoryEntry,
  record,
  selected,
}: Readonly<{
  disabled: boolean;
  onDeleteHistoryEntry: (id: string) => void;
  onSelectHistoryEntry: (id: string) => void;
  record: SimulationRecord;
  selected: boolean;
}>) {
  const environment = getEnvironment(record.environmentId);
  const transaction = record.request.transaction;
  const status =
    'outcome' in record.response
      ? record.response.outcome.status
      : record.response.execution.status === 'SUCCESS'
        ? 'success'
        : record.response.execution.status === 'FAILED'
          ? 'failed'
          : 'rejected';
  const changes = simulationChanges(record.response);

  return (
    <article
      className={cn(
        'border-b border-line transition-colors last:border-b-0',
        selected ? 'bg-brand-50' : 'bg-white',
      )}
    >
      <button
        className={cn(
          'block w-full px-3 pb-2 pt-3 text-left transition-colors disabled:cursor-not-allowed',
          !selected && 'hover:bg-shell-50',
        )}
        disabled={disabled}
        onClick={() => onSelectHistoryEntry(record.id)}
        type="button"
      >
        <div className="flex items-center justify-between gap-3">
          <span className="truncate text-xs font-semibold text-ink-950">
            {environment.shortLabel}
          </span>
          <span className="shrink-0 text-[10px] text-ink-400">
            {formatTimestampLabel(record.createdAt)}
          </span>
        </div>
        <p className="mt-2 truncate font-mono text-[11px] text-ink-600">
          {transaction.to ? shortHex(transaction.to) : 'Contract creation'}
        </p>
      </button>

      <div className="flex min-h-9 items-center justify-between gap-2 px-3 pb-2">
        <div className="flex min-w-0 items-center gap-2 text-[11px]">
          <span className="flex shrink-0 items-center gap-1.5 text-ink-600">
            <span
              className={cn(
                'h-1.5 w-1.5 rounded-full',
                status === 'success'
                  ? 'bg-emerald-500'
                  : status === 'failed' || status === 'reverted'
                    ? 'bg-red-500'
                    : 'bg-amber-500',
              )}
            />
            {status}
          </span>
          <span className="truncate text-ink-400">
            {changes.error
              ? 'Changes unavailable'
              : `${changes.items.length} changes`}
          </span>
        </div>

        <div className="flex shrink-0 items-center gap-0.5">
          <CopyButton
            label="Copy simulation record"
            value={() => formatJson({
              createdAt: record.createdAt,
              environmentId: record.environmentId,
              formValues: record.formValues,
              request: record.request,
              rawResponse: record.rawResponse,
            })}
          />
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                aria-label="Delete history record"
                disabled={disabled}
                onClick={() => onDeleteHistoryEntry(record.id)}
                size="iconSm"
                variant="dangerGhost"
              >
                <Trash2 aria-hidden="true" className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Delete history record</TooltipContent>
          </Tooltip>
        </div>
      </div>
    </article>
  );
}
