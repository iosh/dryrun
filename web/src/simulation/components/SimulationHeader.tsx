import { FlaskConical } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';
import {
  ENVIRONMENT_LIST,
  getEnvironment,
  type EnvironmentId,
} from '../environment.ts';

export interface SimulationHeaderProps {
  environmentId: EnvironmentId;
  isBusy: boolean;
  mobileHistoryAction: ReactNode;
  onEnvironmentChange: (environmentId: EnvironmentId) => void;
}

export function SimulationHeader({
  environmentId,
  isBusy,
  mobileHistoryAction,
  onEnvironmentChange,
}: Readonly<SimulationHeaderProps>) {
  const environment = getEnvironment(environmentId);

  return (
    <header className="border-b border-line bg-white">
      <div className="mx-auto flex min-h-18 max-w-450 flex-col gap-3 px-4 py-3 sm:px-6 md:flex-row md:items-center md:justify-between lg:px-7">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-ink-950 text-white">
              <FlaskConical aria-hidden="true" className="h-4.5 w-4.5" />
            </span>
            <div>
              <p className="text-lg font-semibold leading-5">dryrun</p>
              <p className="mt-1 text-xs text-ink-600">Transaction simulator</p>
            </div>
          </div>
          <div className="text-right md:hidden">
            <p className="font-mono text-[11px] text-ink-600">
              Chain {environment.chainId.toString()}
            </p>
          </div>
        </div>

        <div className="flex min-w-0 items-center gap-4">
          <div className="shrink-0 lg:hidden">{mobileHistoryAction}</div>
          <div
            aria-label="Simulation environment"
            className="grid min-w-0 flex-1 grid-cols-3 rounded-lg border border-line bg-shell-100 p-1 md:w-117.5 md:flex-none"
            role="radiogroup"
          >
            {ENVIRONMENT_LIST.map((option) => {
              const selected = option.id === environmentId;
              return (
                <button
                  aria-checked={selected}
                  className={cn(
                    'min-h-10 min-w-0 rounded-md px-2 text-xs font-medium transition-colors disabled:cursor-not-allowed',
                    selected
                      ? 'bg-white text-ink-950 shadow-sm ring-1 ring-line'
                      : 'text-ink-600 hover:bg-white/70 hover:text-ink-950',
                  )}
                  disabled={isBusy}
                  key={option.id}
                  onClick={() => onEnvironmentChange(option.id)}
                  role="radio"
                  type="button"
                >
                  <span className="block truncate">{option.shortLabel}</span>
                  <span className="mt-0.5 block text-[10px] font-normal text-ink-400">
                    Mainnet
                  </span>
                </button>
              );
            })}
          </div>
          <div className="hidden min-w-20 text-right lg:block">
            <p className="text-xs font-medium text-ink-950">Mainnet</p>
            <p className="mt-1 font-mono text-[11px] text-ink-600">
              Chain {environment.chainId.toString()}
            </p>
          </div>
        </div>
      </div>
    </header>
  );
}
