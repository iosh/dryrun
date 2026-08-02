import type { EnvironmentId } from './environment.ts';
import { getEnvironment } from './environment.ts';
import {
  type RpcEnvelope,
  type RpcErrorPayload,
  type RpcSimulationResponse,
} from './rpc.ts';
import type { SimulationRequest } from './types.ts';

let nextRequestId = 1;

export class TransportError extends Error {
  readonly status?: number;

  constructor(message: string, status?: number) {
    super(message);
    this.name = 'TransportError';
    this.status = status;
  }
}

export class RpcError extends Error {
  readonly payload: RpcErrorPayload;
  readonly rawResponse: unknown;

  constructor(payload: RpcErrorPayload, rawResponse: unknown) {
    super(payload.message);
    this.name = 'RpcError';
    this.payload = payload;
    this.rawResponse = rawResponse;
  }
}

export class InvalidResponseError extends Error {
  readonly rawResponse: unknown;

  constructor(message: string, rawResponse: unknown) {
    super(message);
    this.name = 'InvalidResponseError';
    this.rawResponse = rawResponse;
  }
}

export interface SimulationRpcResult {
  response: RpcSimulationResponse;
  rawResponse: unknown;
}

export function getRpcUrl() {
  return import.meta.env.VITE_DRYRUN_RPC_URL ?? '/rpc';
}

export async function simulateTransaction(
  environmentId: EnvironmentId,
  request: SimulationRequest,
): Promise<SimulationRpcResult> {
  const environment = getEnvironment(environmentId);
  const requestId = nextRequestId++;
  let response: Response;

  try {
    response = await fetch(getRpcUrl(), {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: requestId,
        method: environment.method,
        params: request,
      }),
    });
  } catch {
    throw new TransportError('Unable to reach the simulation service.');
  }

  if (!response.ok) {
    throw new TransportError(
      `The simulation service responded with HTTP ${response.status}.`,
      response.status,
    );
  }

  let payload: RpcEnvelope;
  try {
    payload = await response.json() as RpcEnvelope;
  } catch {
    throw new InvalidResponseError(
      'The simulation service returned invalid JSON.',
      undefined,
    );
  }

  if ('error' in payload) {
    throw new RpcError(payload.error, payload);
  }

  return {
    response: payload.result as RpcSimulationResponse,
    rawResponse: payload,
  };
}
