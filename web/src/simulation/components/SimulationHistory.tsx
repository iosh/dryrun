import { History } from 'lucide-react';

import { cn } from '../../lib/cn.ts';
import { formatTimestampLabel } from '../../lib/formatting.ts';
import { Button } from '../../ui/Button.tsx';
import { Panel } from '../../ui/Panel.tsx';
import type { SimulationRecord } from '../types.ts';

export interface SimulationHistoryProps {
  history: readonly SimulationRecord[];
  isBusy: boolean;
  onNewSimulation: () => void;
  onSelectHistoryEntry: (id: string) => void;
  selectedHistoryId: string | null;
}

export function SimulationHistorySidebar({
  history,
  isBusy,
  onNewSimulation,
  onSelectHistoryEntry,
  selectedHistoryId,
}: Readonly<SimulationHistoryProps>) {
  const hasHistory = history.length > 0;

  return (
    <aside className="hidden h-full min-w-0 flex-col gap-4 border-r border-line bg-shell-150 p-4 lg:flex">
      <div className="space-y-1">
        <h2 className="font-display text-xs font-bold uppercase tracking-[0.14em] text-ink-950">
          History
        </h2>
        <p className="font-mono text-[10px] text-ink-600">
          Saved in your browser
        </p>
      </div>

      <Button
        className="w-full"
        disabled={isBusy}
        onClick={onNewSimulation}
      >
        New Simulation
      </Button>

      <div className="min-h-0 space-y-3 overflow-y-auto pr-1">
        {hasHistory ? (
          <>
            <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-ink-600">
              Recent Runs
            </p>
            {history.map((entry) => (
              <HistoryEntryButton
                entry={entry}
                key={entry.id}
                onSelectHistoryEntry={onSelectHistoryEntry}
                selected={entry.id === selectedHistoryId}
              />
            ))}
          </>
        ) : (
          <HistoryEmptyState />
        )}
      </div>
    </aside>
  );
}

export function SimulationHistoryMobileToolbar({
  isBusy,
  onNewSimulation,
}: Readonly<Pick<SimulationHistoryProps, 'isBusy' | 'onNewSimulation'>>) {
  return (
    <section className="border-b border-line bg-shell-150 p-4 lg:hidden">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="space-y-1">
          <h2 className="font-display text-xs font-bold uppercase tracking-[0.14em] text-ink-950">
            New Simulation
          </h2>
          <p className="font-mono text-[10px] text-ink-600">
            Start a fresh request
          </p>
        </div>

        <Button
          className="sm:w-auto"
          disabled={isBusy}
          onClick={onNewSimulation}
        >
          New Simulation
        </Button>
      </div>
    </section>
  );
}

export function SimulationHistoryMobileList({
  history,
  onSelectHistoryEntry,
  selectedHistoryId,
}: Readonly<
  Pick<
    SimulationHistoryProps,
    'history' | 'onSelectHistoryEntry' | 'selectedHistoryId'
  >
>) {
  const hasHistory = history.length > 0;

  return (
    <section className="space-y-3 lg:hidden">
      <div className="space-y-1">
        <h2 className="font-display text-xs font-bold uppercase tracking-[0.14em] text-ink-950">
          History
        </h2>
        <p className="font-mono text-[10px] text-ink-600">
          Saved in your browser
        </p>
      </div>

      {hasHistory ? (
        <>
          {history.map((entry) => (
            <HistoryEntryButton
              entry={entry}
              key={entry.id}
              onSelectHistoryEntry={onSelectHistoryEntry}
              selected={entry.id === selectedHistoryId}
            />
          ))}
        </>
      ) : (
        <HistoryEmptyState />
      )}
    </section>
  );
}

function HistoryEntryButton({
  entry,
  onSelectHistoryEntry,
  selected,
}: Readonly<{
  entry: SimulationRecord;
  onSelectHistoryEntry: (id: string) => void;
  selected: boolean;
}>) {
  return (
    <button
      className={cn(
        'w-full rounded-[18px] border px-4 py-3 text-left transition',
        selected
          ? 'border-brand-600 bg-white ring-1 ring-brand-600/10 shadow-md'
          : 'border-line bg-white hover:border-brand-600/40 hover:bg-white',
      )}
      onClick={() => onSelectHistoryEntry(entry.id)}
      type="button"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <p
            className={cn(
              'font-display text-[13px] font-bold',
              selected ? 'text-brand-600' : 'text-ink-950',
            )}
          >
            {entry.title}
          </p>
          <p className="text-[11px] text-ink-600">
            {entry.subtitle}
          </p>
        </div>
        <span className="font-mono text-[10px] text-ink-400">
          {formatTimestampLabel(entry.capturedAt)}
        </span>
      </div>
    </button>
  );
}

function HistoryEmptyState() {
  return (
    <Panel className="space-y-3 rounded-[16px] p-4">
      <div className="flex h-9 w-9 items-center justify-center rounded-full bg-emerald-100 text-emerald-700">
        <History className="h-[18px] w-[18px]" strokeWidth={2.25} />
      </div>
      <div className="space-y-1">
        <p className="text-sm font-semibold text-ink-950">
          No history yet
        </p>
        <p className="text-[11px] leading-5 text-ink-600">
          Submitted runs appear here after each simulation.
        </p>
      </div>
    </Panel>
  );
}
