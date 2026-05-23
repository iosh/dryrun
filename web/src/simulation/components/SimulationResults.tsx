import { FileJson2 } from 'lucide-react';

import { formatHexQuantity, shortAddress } from '../../lib/formatting.ts';
import { Badge } from '../../ui/Badge.tsx';
import { MetricCard } from '../../ui/MetricCard.tsx';
import { Panel } from '../../ui/Panel.tsx';
import type { ChangeItemViewModel } from '../changeView.ts';
import type { RunErrorState, SimulationRecord } from '../types.ts';

export interface SimulationResultsProps {
  activeRecord: SimulationRecord | null;
  changeItems: readonly ChangeItemViewModel[];
  rawResponseJson: string;
  runError: RunErrorState | null;
}

export function SimulationResults({
  activeRecord,
  changeItems,
  rawResponseJson,
  runError,
}: Readonly<SimulationResultsProps>) {
  if (!activeRecord) {
    return (
      <Panel className="p-5 sm:p-6">
        <div className="space-y-6">
          {runError ? (
            <Panel className="border-red-200 bg-red-50 p-4 shadow-none">
              <div className="space-y-2">
                <p className="text-sm font-semibold text-red-700">
                  {runError.title}
                </p>
                <p className="text-sm leading-6 text-red-700">
                  {runError.detail}
                </p>
                {runError.subkind ? (
                  <p className="font-mono text-[11px] uppercase tracking-[0.14em] text-red-700">
                    {runError.subkind}
                  </p>
                ) : null}
              </div>
            </Panel>
          ) : null}

          <section className="space-y-3">
            <h2 className="font-display text-[28px] font-bold text-ink-950">
              Execution Summary
            </h2>
            <Panel className="bg-shell-100 p-5 shadow-none">
              <div className="space-y-2">
                <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-ink-600">
                  Awaiting Response
                </p>
                <p className="text-sm leading-6 text-ink-600">
                  Submit a simulation request to inspect execution details and
                  detected changes.
                </p>
              </div>
            </Panel>
          </section>

          <section className="space-y-3">
            <div className="flex items-center justify-between gap-3">
              <h3 className="font-display text-[22px] font-bold text-ink-950">
                Changes
              </h3>
              <Badge tone="slate">Awaiting Response</Badge>
            </div>
            <Panel className="bg-shell-100 p-5 shadow-none">
              <p className="text-sm leading-6 text-ink-600">
                No response has been returned yet.
              </p>
            </Panel>
          </section>

          <section className="space-y-3">
            <div className="flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.16em] text-ink-600">
              <FileJson2 className="h-3.5 w-3.5" strokeWidth={2.25} />
              <p>Raw Response Preview</p>
            </div>
            <Panel className="bg-shell-100 p-4 shadow-none">
              <pre className="max-h-80 overflow-auto font-mono text-[11px] leading-5 text-ink-950">
                {rawResponseJson || 'No response yet.'}
              </pre>
            </Panel>
          </section>
        </div>
      </Panel>
    );
  }

  const { execution } = activeRecord.response;
  const statusTone = execution.status === 'SUCCESS' ? 'blue' : 'amber';

  return (
    <Panel className="p-5 sm:p-6">
      <div className="space-y-6">
        {runError ? (
          <Panel className="border-red-200 bg-red-50 p-4 shadow-none">
            <div className="space-y-2">
              <p className="text-sm font-semibold text-red-700">
                {runError.title}
              </p>
              <p className="text-sm leading-6 text-red-700">
                {runError.detail}
              </p>
              {runError.subkind ? (
                <p className="font-mono text-[11px] uppercase tracking-[0.14em] text-red-700">
                  {runError.subkind}
                </p>
              ) : null}
            </div>
          </Panel>
        ) : null}

        <section className="space-y-4">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <h2 className="font-display text-[28px] font-bold text-ink-950">
                Execution Summary
              </h2>
              <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-ink-600">
                Chain {formatHexQuantity(execution.chainId)} • Block{' '}
                {formatHexQuantity(execution.block.number)}
              </p>
            </div>
            <Badge tone={statusTone}>{execution.status}</Badge>
          </div>

          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <MetricCard
              label="Gas Used"
              value={formatHexQuantity(execution.gasUsed)}
            />
            <MetricCard
              label="Gas Limit"
              value={formatHexQuantity(execution.gasLimit)}
            />
            <MetricCard
              label="Block Number"
              value={formatHexQuantity(execution.block.number)}
            />
            <MetricCard
              label="Status"
              tone="accent"
              value={execution.status === 'SUCCESS' ? 'Success' : 'Failed'}
            />
          </div>

          {execution.error ? (
            <Panel className="border-amber-200 bg-amber-50 p-4 shadow-none">
              <div className="space-y-2">
                <p className="text-sm font-semibold text-amber-800">
                  Execution Failed
                </p>
                <p className="text-sm leading-6 text-amber-800">
                  {execution.error.message}
                </p>
                <div className="flex flex-wrap gap-2 font-mono text-[11px] uppercase tracking-[0.12em] text-amber-800">
                  <span>{execution.error.code}</span>
                  {execution.error.reason ? (
                    <span>{execution.error.reason}</span>
                  ) : null}
                </div>
              </div>
            </Panel>
          ) : null}

          <Panel className="bg-shell-100 p-4 shadow-none">
            <div className="space-y-2">
              <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-ink-600">
                Output Data
              </p>
              <pre className="overflow-x-auto font-mono text-[11px] leading-5 text-ink-950">
                {execution.output}
              </pre>
            </div>
          </Panel>
        </section>

        <section className="space-y-4 rounded-[18px] bg-shell-100 p-4">
          <div className="flex items-center justify-between gap-3">
            <h3 className="font-display text-[22px] font-bold text-ink-950">
              Changes
            </h3>
            <Badge tone="slate">Primary Output</Badge>
          </div>

          <div className="space-y-3">
            {changeItems.length > 0 ? (
              changeItems.map((item, index) => (
                <Panel
                  className="space-y-3 bg-white p-4 shadow-none"
                  key={`${activeRecord.id}:${index}`}
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex min-w-0 items-center gap-3">
                      <Badge tone={item.badgeTone}>{item.badgeLabel}</Badge>
                      <p className="truncate font-display text-[15px] font-semibold text-ink-950">
                        {item.title}
                      </p>
                    </div>
                    {item.value ? (
                      <p className="whitespace-nowrap font-display text-[18px] font-bold text-ink-950">
                        {item.value}
                      </p>
                    ) : null}
                  </div>
                  <p className="text-sm leading-6 text-ink-600">
                    {item.description}
                  </p>
                </Panel>
              ))
            ) : (
              <Panel className="bg-white p-4 shadow-none">
                <p className="text-sm leading-6 text-ink-600">
                  The backend returned no detected changes for this simulation.
                </p>
              </Panel>
            )}
          </div>
        </section>

        <section className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.16em] text-ink-600">
              <FileJson2 className="h-3.5 w-3.5" strokeWidth={2.25} />
              <p>Raw Response Preview</p>
            </div>
            <p className="text-[11px] text-ink-400">
              Block {shortAddress(activeRecord.response.execution.block.hash)}
            </p>
          </div>
          <Panel className="bg-shell-100 p-4 shadow-none">
            <pre className="max-h-80 overflow-auto font-mono text-[11px] leading-5 text-ink-950">
              {rawResponseJson}
            </pre>
          </Panel>
        </section>
      </div>
    </Panel>
  );
}
