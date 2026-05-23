import type { SimulationFormValues } from './types.ts';

export const INITIAL_FORM_VALUES: SimulationFormValues = {
  accessListJson: '',
  calldata: '',
  chainId: '',
  executionBlock: 'latest',
  from: '',
  gasLimit: '',
  gasPriceGwei: '',
  maxFeePerGasGwei: '',
  maxPriorityFeePerGasGwei: '',
  nonce: '',
  to: '',
  txType: 'auto',
  valueEth: '',
};
