import type {
  DryrunSimulateTransactionRequest,
  DryrunSimulateTransactionResponse,
} from './rpc.ts';

export type TxTypeOption =
  | 'auto'
  | 'legacy'
  | 'access-list'
  | 'dynamic-fee';

export interface SimulationFormValues {
  from: string;
  to: string;
  valueEth: string;
  gasLimit: string;
  calldata: string;
  executionBlock: string;
  nonce: string;
  txType: TxTypeOption;
  gasPriceGwei: string;
  maxFeePerGasGwei: string;
  maxPriorityFeePerGasGwei: string;
  accessListJson: string;
}

export interface SimulationRecord {
  id: string;
  title: string;
  subtitle: string;
  capturedAt: string;
  request: DryrunSimulateTransactionRequest;
  response: DryrunSimulateTransactionResponse;
}

export interface RunErrorState {
  title: string;
  detail: string;
  subkind?: string;
}
