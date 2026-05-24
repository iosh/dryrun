export type DryrunRpcAddress = string;
export type DryrunRpcHex = string;
export type DryrunRpcQuantity = string;

interface DryrunNamedDisplay {
  name?: string;
}

interface DryrunNamedSymbolDisplay extends DryrunNamedDisplay {
  symbol?: string;
}

type DryrunTokenDisplay = DryrunNamedDisplay;

interface DryrunNativeDisplay {
  symbol?: string;
  decimals?: number;
}

interface DryrunFungibleTokenDisplay extends DryrunNamedSymbolDisplay {
  decimals?: number;
}

interface DryrunContractAssetBase<TType extends string> {
  type: TType;
  contractAddress: DryrunRpcAddress;
}

interface DryrunContractCollectionBase<TType extends string, TCollectionDisplay> {
  type: TType;
  contractAddress: DryrunRpcAddress;
  collection?: TCollectionDisplay;
}

export interface DryrunAccessListItem {
  address: DryrunRpcAddress;
  storageKeys: readonly DryrunRpcHex[];
}

export type DryrunBlockRef = string | { blockHash: DryrunRpcHex };

export interface DryrunSimulateTransactionOptions {
  stateOverrides?: unknown;
  blockOverrides?: unknown;
  include?: unknown;
}

export interface DryrunTransactionRequest {
  type?: DryrunRpcQuantity;
  chainId?: DryrunRpcQuantity;
  from: DryrunRpcAddress;
  to?: DryrunRpcAddress;
  nonce?: DryrunRpcQuantity;
  gas: DryrunRpcQuantity;
  value?: DryrunRpcQuantity;
  data?: DryrunRpcHex;
  accessList?: readonly DryrunAccessListItem[];
  gasPrice?: DryrunRpcQuantity;
  maxFeePerGas?: DryrunRpcQuantity;
  maxPriorityFeePerGas?: DryrunRpcQuantity;
}

export interface DryrunSimulateTransactionRequest {
  transaction: DryrunTransactionRequest;
  block?: DryrunBlockRef;
  options?: DryrunSimulateTransactionOptions;
}

export interface DryrunExecutionError {
  code: string;
  message: string;
  reason?: string;
}

export interface DryrunSimulatedBlock {
  number: DryrunRpcQuantity;
  hash: DryrunRpcHex;
}

export type DryrunSimulationStatus = 'SUCCESS' | 'FAILED';

export interface DryrunExecution {
  chainId: DryrunRpcQuantity;
  block: DryrunSimulatedBlock;
  status: DryrunSimulationStatus;
  gasUsed: DryrunRpcQuantity;
  gasLimit: DryrunRpcQuantity;
  output: DryrunRpcHex;
  error?: DryrunExecutionError;
}

export interface DryrunNativeAsset {
  type: 'NATIVE';
  display?: DryrunNativeDisplay;
}

export type DryrunErc20Asset = DryrunContractAssetBase<'ERC20'> & {
  display?: DryrunFungibleTokenDisplay;
};

export type DryrunErc721Asset = DryrunContractAssetBase<'ERC721'> & {
  tokenId: string;
  collection?: DryrunNamedSymbolDisplay;
  token?: DryrunTokenDisplay;
};

export type DryrunErc1155Asset = DryrunContractAssetBase<'ERC1155'> & {
  tokenId: string;
  collection?: DryrunNamedDisplay;
  token?: DryrunTokenDisplay;
};

export type DryrunAsset =
  | DryrunNativeAsset
  | DryrunErc20Asset
  | DryrunErc721Asset
  | DryrunErc1155Asset;

export type DryrunErc721Collection = DryrunContractCollectionBase<
  'ERC721',
  DryrunNamedSymbolDisplay
>;

export type DryrunErc1155Collection = DryrunContractCollectionBase<
  'ERC1155',
  DryrunNamedDisplay
>;

export type DryrunCollection =
  | DryrunErc721Collection
  | DryrunErc1155Collection;

export type DryrunChange =
  | {
      kind: 'TRANSFER';
      asset: DryrunAsset;
      from: DryrunRpcAddress;
      to: DryrunRpcAddress;
      amount?: string;
    }
  | {
      kind: 'MINT';
      asset: DryrunAsset;
      to: DryrunRpcAddress;
      amount?: string;
    }
  | {
      kind: 'BURN';
      asset: DryrunAsset;
      from: DryrunRpcAddress;
      amount?: string;
    }
  | {
      kind: 'APPROVAL';
      asset: DryrunAsset;
      owner: DryrunRpcAddress;
      spender: DryrunRpcAddress;
      amount?: string;
    }
  | {
      kind: 'APPROVAL_FOR_ALL';
      collection: DryrunCollection;
      owner: DryrunRpcAddress;
      operator: DryrunRpcAddress;
      approved: boolean;
    };

export interface DryrunSimulateTransactionResponse {
  execution: DryrunExecution;
  changes: readonly DryrunChange[];
}

export interface DryrunRpcErrorData {
  subkind?: string;
  details: string;
}

export interface DryrunJsonRpcError {
  code: number;
  message: string;
  data?: DryrunRpcErrorData;
}
