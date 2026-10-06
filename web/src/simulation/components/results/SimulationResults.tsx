import { Activity, AlertTriangle, LoaderCircle } from 'lucide-react';

import { formatJson } from '../../../lib/formatting.ts';
import { CopyButton } from '../../../ui/CopyButton.tsx';
import type {
  RequestErrorState,
  SimulationRecord,
} from '../../types.ts';
import { ChangesList } from './ChangesList.tsx';
import {
  ExecutionDetails,
  ExecutionFailure,
  ExecutionSummary,
} from './ExecutionPanels.tsx';
import { CoreLimitations } from './CoreLimitations.tsx';
import {
  RawJsonDetails,
  ResultShell,
} from './ResultPrimitives.tsx';
import { useAddressHighlight } from './useAddressHighlight.ts';

export interface SimulationResultsProps {
  activeRecord: SimulationRecord | null;
  isRunning: boolean;
  runError: RequestErrorState | null;
}

export function SimulationResults({
  activeRecord,
  isRunning,
  runError,
}: Readonly<SimulationResultsProps>) {
  if (isRunning) {
    return (
      <ResultShell>
        <div className="flex min-h-64 flex-col items-center justify-center text-center">
          <LoaderCircle
            aria-hidden="true"
            className="h-6 w-6 animate-spin text-brand-600"
          />
          <p className="mt-4 text-sm font-medium">Simulating transaction</p>
        </div>
      </ResultShell>
    );
  }

  if (runError) {
    return <RequestError error={runError} />;
  }

  if (!activeRecord) {
    return (
      <ResultShell>
        <div className="flex min-h-64 flex-col items-center justify-center text-center">
          <span className="flex h-11 w-11 items-center justify-center rounded-lg border border-line bg-shell-100 text-ink-400">
            <Activity aria-hidden="true" className="h-5 w-5" />
          </span>
          <h2 className="mt-4 text-base font-semibold">No result yet</h2>
        </div>
      </ResultShell>
    );
  }

  return <SimulationResult key={activeRecord.id} record={activeRecord} />;
}

function SimulationResult({ record }: Readonly<{ record: SimulationRecord }>) {
  const addressHighlight = useAddressHighlight();

  return (
    <div
      className="space-y-5"
      onClick={(event) => {
        if (
          event.target instanceof Element &&
          !event.target.closest('[data-address-value]')
        ) {
          addressHighlight.clearPinnedAddress();
        }
      }}
    >
      <ExecutionSummary record={record} />
      {'epoch' in record.response ? <CoreLimitations limitations={record.response.limitations} /> : null}
      <ExecutionFailure outcome={record.response.outcome} />
      <ExecutionDetails record={record} addressHighlight={addressHighlight} />
      <RawJsonDetails label="Completed transaction" value={record.response.transaction} />
      <ChangesList addressHighlight={addressHighlight} record={record} />

      <RawJsonDetails label="Raw RPC response" value={record.rawResponse} />
    </div>
  );
}

function RequestError({ error }: Readonly<{ error: RequestErrorState }>) {
  return (
    <ResultShell>
      <div className="border-l-2 border-red-500 bg-red-50 px-4 py-4">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 gap-3">
            <AlertTriangle
              aria-hidden="true"
              className="mt-0.5 h-5 w-5 shrink-0 text-red-700"
            />
            <div className="min-w-0">
              <h2 className="text-base font-semibold text-red-900">
                {error.title}
              </h2>
              <p className="mt-2 text-sm leading-6 text-red-800">
                {error.detail}
              </p>
            </div>
          </div>
          <CopyButton
            label="Copy error report"
            tone="error"
            value={() => formatErrorReport(error)}
          />
        </div>
      </div>
      {error.rawResponse !== undefined ? (
        <div className="mt-5">
          <RawJsonDetails label="Raw RPC response" value={error.rawResponse} />
        </div>
      ) : null}
    </ResultShell>
  );
}

function formatErrorReport(error: RequestErrorState) {
  return formatJson({
    environmentId: error.context.environmentId,
    error: {
      detail: error.detail,
      kind: error.kind,
      title: error.title,
    },
    formValues: error.context.formValues,
    request: error.context.request,
    ...(error.rawResponse !== undefined
      ? { rawResponse: error.rawResponse }
      : {}),
  });
}
