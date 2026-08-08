import { useState } from 'react';

import {
  InvalidResponseError,
  RpcError,
  simulateTransaction,
  TransportError,
} from './client.ts';
import type { EnvironmentId } from './environment.ts';
import {
  addSimulationHistory,
  loadSimulationHistory,
  removeSimulationHistory,
} from './history.ts';
import { createInitialFormValues, parseSimulationForm } from './request.ts';
import { useSimulationForm } from './form.ts';
import type {
  RequestErrorState,
  SimulationFormValues,
  SimulationRecord,
} from './types.ts';

export function useSimulationPage() {
  const [environmentId, setEnvironmentId] =
    useState<EnvironmentId>('ethereum-mainnet');
  const [history, setHistory] = useState(loadSimulationHistory);
  const [activeRecord, setActiveRecord] =
    useState<SimulationRecord | null>(null);
  const [runError, setRunError] = useState<RequestErrorState | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  const form = useSimulationForm(environmentId, runSimulation);

  async function runSimulation(formValues: SimulationFormValues) {
    const parsed = parseSimulationForm(environmentId, formValues);
    if (!parsed.request) return;

    const requestedEnvironment = environmentId;
    const requestContext: RequestErrorState['context'] = {
      environmentId: requestedEnvironment,
      formValues: { ...formValues },
      request: parsed.request,
    };
    setIsRunning(true);
    setRunError(null);
    setActiveRecord(null);

    try {
      const result = await simulateTransaction(
        requestedEnvironment,
        parsed.request,
      );
      const record: SimulationRecord = {
        createdAt: new Date().toISOString(),
        id: crypto.randomUUID(),
        rawResponse: result.rawResponse,
        response: result.response,
        ...requestContext,
      };

      setActiveRecord(record);
      setHistory((current) => addSimulationHistory(current, record));
    } catch (error) {
      setRunError(toRequestErrorState(error, requestContext));
    } finally {
      setIsRunning(false);
    }
  }

  function changeEnvironment(nextEnvironmentId: EnvironmentId) {
    if (isRunning || nextEnvironmentId === environmentId) return;
    setEnvironmentId(nextEnvironmentId);
    form.reset(createInitialFormValues());
    setActiveRecord(null);
    setRunError(null);
  }

  function startNewSimulation() {
    if (isRunning) return;
    form.reset(createInitialFormValues());
    setActiveRecord(null);
    setRunError(null);
  }

  function selectHistoryEntry(id: string) {
    if (isRunning) return;
    const record = history.find((entry) => entry.id === id);
    if (!record) return;

    setEnvironmentId(record.environmentId);
    form.reset(record.formValues);
    setActiveRecord(record);
    setRunError(null);
  }

  function deleteHistoryEntry(id: string) {
    if (isRunning) return;
    setHistory((current) => removeSimulationHistory(current, id));
    setActiveRecord((current) => current?.id === id ? null : current);
  }

  return {
    activeRecord,
    changeEnvironment,
    deleteHistoryEntry,
    environmentId,
    form,
    history,
    isRunning,
    runError,
    selectHistoryEntry,
    startNewSimulation,
  };
}

function toRequestErrorState(
  error: unknown,
  context: RequestErrorState['context'],
): RequestErrorState {
  if (error instanceof RpcError) {
    return {
      context,
      detail: error.payload.message,
      kind: 'rpc',
      rawResponse: error.rawResponse,
      title: rpcErrorTitle(error.payload.code),
    };
  }

  if (error instanceof TransportError) {
    return {
      context,
      detail: error.message,
      kind: 'transport',
      title: 'Service unavailable',
    };
  }

  if (error instanceof InvalidResponseError) {
    return {
      context,
      detail: error.message,
      kind: 'invalid-response',
      rawResponse: error.rawResponse,
      title: 'Invalid service response',
    };
  }

  return {
    context,
    detail: error instanceof Error ? error.message : 'The request failed.',
    kind: 'transport',
    title: 'Simulation failed',
  };
}

function rpcErrorTitle(code: number) {
  switch (code) {
    case -32602:
      return 'Invalid request';
    case -32603:
      return 'Server error';
    case -32004:
      return 'Not supported';
    default:
      return 'RPC error';
  }
}
