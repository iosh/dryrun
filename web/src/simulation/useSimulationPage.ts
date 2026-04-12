import { useMutation } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import { formatHexQuantity, formatJson } from '../lib/formatting.ts';
import {
  DryrunRpcError,
  DryrunTransportError,
  simulateTransaction,
} from './client.ts';
import { toChangeItemViewModel } from './changeView.ts';
import { INITIAL_FORM_VALUES } from './defaults.ts';
import { useSimulationForm } from './form.ts';
import {
  hydrateSimulationFormValues,
  serializeSimulationRequest,
} from './requestCodec.ts';
import type {
  RunErrorState,
  SimulationRecord,
  SimulationFormValues,
} from './types.ts';

const HISTORY_STORAGE_KEY = 'dryrun:web:simulation-history';
const MAX_HISTORY_ITEMS = 8;

export function useSimulationPage() {
  const [activeRecord, setActiveRecord] = useState<SimulationRecord | null>(null);
  const [history, setHistory] = useState<SimulationRecord[]>(loadHistory);
  const [selectedHistoryId, setSelectedHistoryId] = useState<string | null>(null);
  const [runError, setRunError] = useState<RunErrorState | null>(null);

  const simulationMutation = useMutation({
    mutationFn: simulateTransaction,
  });
  const form = useSimulationForm(submitFormValues, clearRunError);

  useEffect(() => {
    window.localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(history));
  }, [history]);

  async function submitFormValues(formValues: SimulationFormValues) {
    const request = serializeSimulationRequest(formValues);

    if (!request) {
      clearActiveResult();
      setRunError({
        detail:
          'Client-side validation and request serialization are out of sync.',
        title: 'Unable to build request',
      });
      return;
    }

    setRunError(null);

    try {
      const response = await simulationMutation.mutateAsync(request);
      const record = createSimulationRecord(request, response);

      setActiveRecord(record);
      setSelectedHistoryId(record.id);
      setHistory((currentHistory) => [
        record,
        ...currentHistory.filter((entry) => entry.id !== record.id),
      ].slice(0, MAX_HISTORY_ITEMS));
    } catch (error) {
      clearActiveResult();
      setRunError(normalizeRunError(error));
    }
  }

  function clearRunError() {
    setRunError(null);
  }

  function clearActiveResult() {
    setActiveRecord(null);
    setSelectedHistoryId(null);
  }

  function resetComposer() {
    form.reset(INITIAL_FORM_VALUES, { keepDefaultValues: true });
    setRunError(null);
  }

  function startNewSimulation() {
    resetComposer();
    clearActiveResult();
  }

  function selectHistoryEntry(id: string) {
    const nextRecord = history.find((entry) => entry.id === id);

    if (!nextRecord) {
      return;
    }

    setSelectedHistoryId(id);
    setActiveRecord(nextRecord);
    form.reset(hydrateSimulationFormValues(nextRecord.request), {
      keepDefaultValues: true,
    });
    setRunError(null);
  }

  return {
    activeRecord,
    activeResponseJson: activeRecord ? formatJson(activeRecord.response) : '',
    changeItems: activeRecord
      ? activeRecord.response.changes.map(toChangeItemViewModel)
      : [],
    form,
    history,
    isRunning: simulationMutation.isPending,
    resetComposer,
    runError,
    selectedHistoryId,
    selectHistoryEntry,
    startNewSimulation,
  };
}

function loadHistory() {
  if (typeof window === 'undefined') {
    return [];
  }

  const storedValue = window.localStorage.getItem(HISTORY_STORAGE_KEY);

  if (!storedValue) {
    return [];
  }

  try {
    const parsed = JSON.parse(storedValue) as unknown;
    return Array.isArray(parsed)
      ? parsed.filter(isStoredSimulationRecord).slice(0, MAX_HISTORY_ITEMS)
      : [];
  } catch {
    return [];
  }
}

function createSimulationRecord(
  request: SimulationRecord['request'],
  response: SimulationRecord['response'],
) {
  const firstChange = response.changes[0]
    ? toChangeItemViewModel(response.changes[0])
    : null;
  const capturedAt = new Date().toISOString();

  return {
    capturedAt,
    id: crypto.randomUUID(),
    request,
    response,
    subtitle: firstChange?.title ?? 'No detected changes',
    title: `Block ${formatHexQuantity(response.execution.block.number)} (${response.execution.status})`,
  };
}

function isStoredSimulationRecord(value: unknown): value is SimulationRecord {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as {
    id?: unknown;
    title?: unknown;
    subtitle?: unknown;
    capturedAt?: unknown;
    request?: unknown;
    response?: unknown;
    source?: unknown;
  };

  if (candidate.id === 'demo-record' || candidate.source === 'sample') {
    return false;
  }

  return (
    typeof candidate.id === 'string' &&
    typeof candidate.title === 'string' &&
    typeof candidate.subtitle === 'string' &&
    typeof candidate.capturedAt === 'string' &&
    hasStoredRequest(candidate.request) &&
    hasStoredResponse(candidate.response)
  );
}

function hasStoredRequest(value: unknown): value is SimulationRecord['request'] {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as {
    transaction?: { from?: unknown; gas?: unknown } | null;
  };

  return (
    !!candidate.transaction &&
    typeof candidate.transaction.from === 'string' &&
    typeof candidate.transaction.gas === 'string'
  );
}

function hasStoredResponse(value: unknown): value is SimulationRecord['response'] {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as {
    execution?: {
      chainId?: unknown;
      gasUsed?: unknown;
      gasLimit?: unknown;
      output?: unknown;
      status?: unknown;
      block?: { number?: unknown; hash?: unknown } | null;
    } | null;
    changes?: unknown;
  };

  return (
    !!candidate.execution &&
    typeof candidate.execution.chainId === 'string' &&
    typeof candidate.execution.gasUsed === 'string' &&
    typeof candidate.execution.gasLimit === 'string' &&
    typeof candidate.execution.output === 'string' &&
    (candidate.execution.status === 'SUCCESS' ||
      candidate.execution.status === 'FAILED') &&
    !!candidate.execution.block &&
    typeof candidate.execution.block.number === 'string' &&
    typeof candidate.execution.block.hash === 'string' &&
    Array.isArray(candidate.changes)
  );
}

function normalizeRunError(error: unknown): RunErrorState {
  if (error instanceof DryrunRpcError) {
    return {
      detail: error.rpcError.data?.details ?? 'The RPC server rejected the request.',
      subkind: error.rpcError.data?.subkind,
      title: error.message,
    };
  }

  if (error instanceof DryrunTransportError) {
    return {
      detail:
        'The simulation page could not reach the RPC endpoint. Check the backend server and proxy configuration.',
      title: error.message,
    };
  }

  return {
    detail: 'The simulation request failed unexpectedly.',
    title: 'Unexpected error',
  };
}
