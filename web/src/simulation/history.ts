import { ENVIRONMENTS, type EnvironmentId } from './environment.ts';
import { createInitialFormValues, parseSimulationForm } from './request.ts';
import { parseRpcEnvelope, parseSimulationResponse } from './response.ts';
import type { SimulationFormValues, SimulationRecord } from './types.ts';

const HISTORY_KEY = 'dryrun.simulation-history.v6';
export const HISTORY_LIMIT = 30;

type StoredSimulationRecord = Pick<SimulationRecord,
  'id' | 'createdAt' | 'environmentId' | 'formValues' | 'rawResponse'>;

interface StoredHistoryPayload {
  version: 6;
  records: StoredSimulationRecord[];
}

export function loadSimulationHistory(): SimulationRecord[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) return [];
    const payload: unknown = JSON.parse(raw);
    if (!isObject(payload) || payload.version !== 6 || !Array.isArray(payload.records)) return [];
    return payload.records.flatMap(restoreSimulationRecord).slice(0, HISTORY_LIMIT);
  } catch {
    return [];
  }
}

export function addSimulationHistory(current: readonly SimulationRecord[], record: SimulationRecord) {
  const records = [record, ...current].slice(0, HISTORY_LIMIT);
  persistSimulationHistory(records);
  return records;
}

export function removeSimulationHistory(current: readonly SimulationRecord[], recordId: string) {
  const records = current.filter((record) => record.id !== recordId);
  persistSimulationHistory(records);
  return records;
}

function restoreSimulationRecord(value: unknown): SimulationRecord[] {
  try {
    if (!isObject(value) || typeof value.id !== 'string' || typeof value.createdAt !== 'string' ||
      !Number.isFinite(Date.parse(value.createdAt)) || typeof value.environmentId !== 'string' ||
      !Object.hasOwn(ENVIRONMENTS, value.environmentId) || !isObject(value.formValues)) return [];
    const formValues = value.formValues;
    if (Object.keys(createInitialFormValues()).some((key) => typeof formValues[key] !== 'string') ||
      !['auto', 'legacy', 'access-list', 'dynamic-fee'].includes(formValues.txType as string) ||
      !['latest', 'safe', 'finalized', 'number', 'hash'].includes(formValues.contextMode as string)) return [];
    const environmentId = value.environmentId as EnvironmentId;
    const savedForm = formValues as unknown as SimulationFormValues;
    // Saved form values use the same input validation and request builder as submission.
    const parsed = parseSimulationForm(environmentId, savedForm);
    if (!parsed.request) return [];
    const envelope = parseRpcEnvelope(value.rawResponse);
    if (!('result' in envelope)) return [];
    return [{
      id: value.id,
      createdAt: value.createdAt,
      environmentId,
      formValues: savedForm,
      request: parsed.request,
      response: parseSimulationResponse(envelope.result, environmentId),
      rawResponse: value.rawResponse,
    }];
  } catch {
    // One incompatible or damaged record must not hide the other saved simulations.
    return [];
  }
}

function persistSimulationHistory(records: readonly SimulationRecord[]) {
  try {
    const payload: StoredHistoryPayload = {
      version: 6,
      records: records.map(({ id, createdAt, environmentId, formValues, rawResponse }) => ({
        id, createdAt, environmentId, formValues, rawResponse,
      })),
    };
    localStorage.setItem(HISTORY_KEY, JSON.stringify(payload));
  } catch {
    // A completed simulation remains usable even when browser storage is full.
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
