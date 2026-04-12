import { getAddress } from 'viem';

import type {
  DryrunJsonRpcError,
  DryrunSimulateTransactionRequest,
  DryrunSimulateTransactionResponse,
} from './rpc.ts';

const DEFAULT_RPC_URL = '/rpc';
const METHOD_NAME = 'dryrun_evm_simulateTransaction';

let nextRpcId = 1;

export class DryrunTransportError extends Error {
  readonly status?: number;

  constructor(message: string, status?: number) {
    super(message);
    this.name = 'DryrunTransportError';
    this.status = status;
  }
}

export class DryrunRpcError extends Error {
  readonly rpcError: DryrunJsonRpcError;

  constructor(rpcError: DryrunJsonRpcError) {
    super(rpcError.message);
    this.name = 'DryrunRpcError';
    this.rpcError = rpcError;
  }
}

export function getDryrunRpcUrl() {
  return import.meta.env.VITE_DRYRUN_RPC_URL ?? DEFAULT_RPC_URL;
}

export async function simulateTransaction(
  request: DryrunSimulateTransactionRequest,
) {
  return callRpcMethod<DryrunSimulateTransactionResponse>(METHOD_NAME, request);
}

async function callRpcMethod<TResponse>(
  method: string,
  params: object,
) {
  let response: Response;

  try {
    response = await fetch(getDryrunRpcUrl(), {
      body: JSON.stringify({
        id: nextRpcId++,
        jsonrpc: '2.0',
        method,
        params,
      }),
      headers: {
        'content-type': 'application/json',
      },
      method: 'POST',
    });
  } catch {
    throw new DryrunTransportError('Unable to reach RPC endpoint');
  }

  if (!response.ok) {
    throw new DryrunTransportError(
      `RPC endpoint responded with ${response.status}`,
      response.status,
    );
  }

  let payload: unknown;

  try {
    payload = await response.json();
  } catch {
    throw new DryrunTransportError(
      'RPC endpoint returned invalid JSON',
      response.status,
    );
  }

  if (isJsonRpcErrorPayload(payload)) {
    throw new DryrunRpcError(payload.error);
  }

  if (!isJsonRpcResultPayload<TResponse>(payload)) {
    throw new DryrunTransportError(
      'RPC endpoint returned an invalid JSON-RPC payload',
      response.status,
    );
  }

  return payload.result;
}

export function normalizeRpcAddress(value: string) {
  return getAddress(value);
}

function isJsonRpcErrorPayload(
  payload: unknown,
): payload is { error: DryrunJsonRpcError; id: number; jsonrpc: '2.0' } {
  return isJsonRpcBasePayload(payload) && 'error' in payload;
}

function isJsonRpcResultPayload<TResponse>(
  payload: unknown,
): payload is { id: number; jsonrpc: '2.0'; result: TResponse } {
  return isJsonRpcBasePayload(payload) && 'result' in payload;
}

function isJsonRpcBasePayload(
  payload: unknown,
): payload is { id: number; jsonrpc: '2.0' } {
  return (
    !!payload &&
    typeof payload === 'object' &&
    'id' in payload &&
    'jsonrpc' in payload
  );
}
