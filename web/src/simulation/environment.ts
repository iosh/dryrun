export type EnvironmentId =
  | 'ethereum-mainnet'
  | 'conflux-espace-mainnet'
  | 'conflux-core-mainnet';

export type SimulationSpace = 'ethereum' | 'espace' | 'core-space';

export interface EnvironmentDefinition {
  id: EnvironmentId;
  label: string;
  shortLabel: string;
  networkLabel: string;
  method: string;
  chainId: bigint;
  nativeSymbol: 'ETH' | 'CFX';
  feeUnit: 'Gwei' | 'GDrip';
  addressKind: 'hex' | 'core-base32';
  addressPlaceholder: string;
  coreAddressPrefix?: string;
  contextKind: 'block' | 'epoch';
  space: SimulationSpace;
}

export const ENVIRONMENTS = {
  'ethereum-mainnet': {
    id: 'ethereum-mainnet',
    label: 'Ethereum',
    shortLabel: 'Ethereum',
    networkLabel: 'Mainnet',
    method: 'dryrun_evm_simulateTransaction',
    chainId: 1n,
    nativeSymbol: 'ETH',
    feeUnit: 'Gwei',
    addressKind: 'hex',
    addressPlaceholder: '0x...',
    contextKind: 'block',
    space: 'ethereum',
  },
  'conflux-espace-mainnet': {
    id: 'conflux-espace-mainnet',
    label: 'Conflux eSpace',
    shortLabel: 'eSpace',
    networkLabel: 'Mainnet',
    method: 'dryrun_conflux_espace_simulateTransaction',
    chainId: 1030n,
    nativeSymbol: 'CFX',
    feeUnit: 'GDrip',
    addressKind: 'hex',
    addressPlaceholder: '0x...',
    contextKind: 'block',
    space: 'espace',
  },
  'conflux-core-mainnet': {
    id: 'conflux-core-mainnet',
    label: 'Conflux Core Space',
    shortLabel: 'Core Space',
    networkLabel: 'Mainnet',
    method: 'dryrun_conflux_coreSpace_simulateTransaction',
    chainId: 1029n,
    nativeSymbol: 'CFX',
    feeUnit: 'GDrip',
    addressKind: 'core-base32',
    addressPlaceholder: 'cfx:...',
    coreAddressPrefix: 'cfx:',
    contextKind: 'epoch',
    space: 'core-space',
  },
} as const satisfies Record<EnvironmentId, EnvironmentDefinition>;

export const ENVIRONMENT_LIST = Object.values(ENVIRONMENTS);

export function getEnvironment(id: EnvironmentId): EnvironmentDefinition {
  return ENVIRONMENTS[id];
}
