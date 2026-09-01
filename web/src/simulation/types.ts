import type { EnvironmentId } from './environment.ts';
import type {
  CoreChange,
  EspaceChange,
  EvmChange,
  RpcSimulationResponse,
} from './rpc.ts';

export type TxTypeOption =
  | 'auto'
  | 'legacy'
  | 'access-list'
  | 'dynamic-fee';

export type ContextMode =
  | 'latest'
  | 'safe'
  | 'finalized'
  | 'number'
  | 'hash';

export interface SimulationFormValues {
  from: string;
  to: string;
  value: string;
  data: string;
  contextMode: ContextMode;
  contextNumber: string;
  nonce: string;
  gasLimit: string;
  txType: TxTypeOption;
  gasPrice: string;
  maxFeePerGas: string;
  maxPriorityFeePerGas: string;
  accessListJson: string;
  storageLimit: string;
  epochHeight: string;
}

export interface RpcAccessListItem {
  address: string;
  storageKeys: string[];
}

export interface HexTransactionRequest {
  type?: string;
  chainId: string;
  from: string;
  to?: string;
  nonce?: string;
  gas?: string;
  value?: string;
  data?: string;
  accessList?: RpcAccessListItem[];
  gasPrice?: string;
  maxFeePerGas?: string;
  maxPriorityFeePerGas?: string;
}

export interface HexSimulationRequest {
  transaction: HexTransactionRequest;
  block: string;
}

export interface CoreTransactionRequest extends HexTransactionRequest {
  storageLimit?: string;
  epochHeight?: string;
}

export interface CoreSimulationRequest {
  transaction: CoreTransactionRequest;
  epoch: string;
}

export type SimulationRequest = HexSimulationRequest | CoreSimulationRequest;

export type SimulationResponse = RpcSimulationResponse;

export type SimulationChange = EvmChange | EspaceChange | CoreChange;

export interface SimulationChanges {
  items: readonly SimulationChange[];
  error: string | null;
}

export function simulationChanges(
  response: RpcSimulationResponse,
): SimulationChanges {
  const changes = response.changes;
  if (Array.isArray(changes)) {
    return { items: changes, error: null };
  }

  if (changes.status === 'unavailable') {
    return { items: [], error: changes.error };
  }

  return {
    items: changes.items,
    error: null,
  };
}

export interface SimulationRecord {
  id: string;
  createdAt: string;
  environmentId: EnvironmentId;
  formValues: SimulationFormValues;
  request: SimulationRequest;
  response: SimulationResponse;
  rawResponse: unknown;
}

export interface RequestErrorState {
  context: Pick<
    SimulationRecord,
    'environmentId' | 'formValues' | 'request'
  >;
  kind: 'transport' | 'rpc' | 'invalid-response';
  title: string;
  detail: string;
  rawResponse?: unknown;
}

export interface ParsedFormResult {
  fieldIssues: Partial<Record<keyof SimulationFormValues, string>>;
  formIssues: string[];
  request?: SimulationRequest;
}

export function isCoreEnvironment(
  environmentId: EnvironmentId,
): environmentId is 'conflux-core-mainnet' {
  return environmentId === 'conflux-core-mainnet';
}
