import type { RpcResultEnvelope } from './rpc.ts';
import type {
  SimulationRecord,
  SimulationResponse,
} from './types.ts';

const HISTORY_KEY = 'dryrun.simulation-history.v3';
export const HISTORY_LIMIT = 30;

type StoredSimulationRecord = Omit<SimulationRecord, 'response'>;

interface StoredHistoryPayload {
  version: 3;
  records: StoredSimulationRecord[];
}

export function loadSimulationHistory(): SimulationRecord[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) return [];

    const payload = JSON.parse(raw) as StoredHistoryPayload;
    if (payload.version !== 3 || !Array.isArray(payload.records)) return [];

    return payload.records
      .map(restoreSimulationRecord)
      .filter((record) => record !== null)
      .slice(0, HISTORY_LIMIT);
  } catch {
    return [];
  }
}

export function addSimulationHistory(
  current: readonly SimulationRecord[],
  record: SimulationRecord,
) {
  const records = [record, ...current].slice(0, HISTORY_LIMIT);
  persistSimulationHistory(records);
  return records;
}

export function removeSimulationHistory(
  current: readonly SimulationRecord[],
  recordId: string,
) {
  const records = current.filter((record) => record.id !== recordId);
  persistSimulationHistory(records);
  return records;
}

function restoreSimulationRecord(
  record: StoredSimulationRecord,
): SimulationRecord | null {
  const envelope = record.rawResponse as RpcResultEnvelope;
  if (
    record.environmentId === 'conflux-espace-mainnet' &&
    (typeof envelope.result !== 'object' ||
      envelope.result === null ||
      !('outcome' in envelope.result))
  ) {
    return null;
  }
  return {
    ...record,
    response: envelope.result as SimulationResponse,
  };
}

function persistSimulationHistory(records: readonly SimulationRecord[]) {
  try {
    const payload: StoredHistoryPayload = {
      records: records.map(toStoredSimulationRecord),
      version: 3,
    };
    localStorage.setItem(HISTORY_KEY, JSON.stringify(payload));
  } catch {
    // A completed simulation remains usable even when browser storage is full.
  }
}

function toStoredSimulationRecord(
  record: SimulationRecord,
): StoredSimulationRecord {
  return {
    createdAt: record.createdAt,
    environmentId: record.environmentId,
    formValues: record.formValues,
    id: record.id,
    rawResponse: record.rawResponse,
    request: record.request,
  };
}
