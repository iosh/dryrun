interface AssetMetadata {
  name?: string;
  symbol?: string;
}

interface FungibleAssetMetadata extends AssetMetadata {
  decimals?: number;
}

interface EvmNativeCurrency {
  name: string;
  symbol: string;
  decimals: number;
}

export interface EvmNativeTransferChange extends EvmNativeCurrency {
  changeType: 'NATIVE_TRANSFER';
  from: string;
  to: string;
  rawAmount: string;
}

export interface EvmSelfDestructBurnChange extends EvmNativeCurrency {
  changeType: 'SELF_DESTRUCT_BURN';
  contractAddress: string;
  rawAmount: string;
}

export interface EvmWrappedNativeDepositChange
  extends FungibleAssetMetadata {
  changeType: 'WRAPPED_NATIVE_DEPOSIT';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface EvmWrappedNativeWithdrawalChange
  extends FungibleAssetMetadata {
  changeType: 'WRAPPED_NATIVE_WITHDRAWAL';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface EvmErc20TransferChange extends FungibleAssetMetadata {
  changeType: 'ERC20_TRANSFER';
  contractAddress: string;
  from: string;
  to: string;
  rawAmount: string;
}

export interface EvmErc20ApprovalChange extends FungibleAssetMetadata {
  changeType: 'ERC20_APPROVAL';
  contractAddress: string;
  owner: string;
  spender: string;
  approvedAmount: string;
}

export interface EvmErc721TransferChange extends AssetMetadata {
  changeType: 'ERC721_TRANSFER';
  contractAddress: string;
  from: string;
  to: string;
  tokenId: string;
}

export interface EvmErc721ApprovalChange extends AssetMetadata {
  changeType: 'ERC721_APPROVAL';
  contractAddress: string;
  owner: string;
  approvedAddress: string | null;
  tokenId: string;
}

export interface EvmOperatorApprovalChange {
  changeType: 'OPERATOR_APPROVAL';
  contractAddress: string;
  owner: string;
  operator: string;
  approved: boolean;
}

export interface EvmErc1155TransferSingleChange {
  changeType: 'ERC1155_TRANSFER_SINGLE';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface Erc1155TransferItem {
  tokenId: string;
  rawAmount: string;
}

export interface EvmErc1155TransferBatchChange {
  changeType: 'ERC1155_TRANSFER_BATCH';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  items: Erc1155TransferItem[];
}

export type EvmChange =
  | EvmNativeTransferChange
  | EvmSelfDestructBurnChange
  | EvmWrappedNativeDepositChange
  | EvmWrappedNativeWithdrawalChange
  | EvmErc20TransferChange
  | EvmErc20ApprovalChange
  | EvmErc721TransferChange
  | EvmErc721ApprovalChange
  | EvmOperatorApprovalChange
  | EvmErc1155TransferSingleChange
  | EvmErc1155TransferBatchChange;

interface EspaceNativeCurrency {
  name: string;
  symbol: string;
  decimals: number;
}

export interface EspaceNativeTransferChange extends EspaceNativeCurrency {
  changeType: 'NATIVE_TRANSFER';
  from: string;
  to: string;
  rawAmount: string;
}

export interface EspaceSelfDestructBurnChange extends EspaceNativeCurrency {
  changeType: 'SELF_DESTRUCT_BURN';
  contractAddress: string;
  rawAmount: string;
}

export interface EspaceWrappedNativeDepositChange
  extends FungibleAssetMetadata {
  changeType: 'WRAPPED_NATIVE_DEPOSIT';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface EspaceWrappedNativeWithdrawalChange
  extends FungibleAssetMetadata {
  changeType: 'WRAPPED_NATIVE_WITHDRAWAL';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface EspaceErc20TransferChange extends FungibleAssetMetadata {
  changeType: 'ERC20_TRANSFER';
  contractAddress: string;
  from: string;
  to: string;
  rawAmount: string;
}

export interface EspaceErc20ApprovalChange extends FungibleAssetMetadata {
  changeType: 'ERC20_APPROVAL';
  contractAddress: string;
  owner: string;
  spender: string;
  approvedAmount: string;
}

export interface EspaceErc721TransferChange extends AssetMetadata {
  changeType: 'ERC721_TRANSFER';
  contractAddress: string;
  from: string;
  to: string;
  tokenId: string;
}

export interface EspaceErc721ApprovalChange extends AssetMetadata {
  changeType: 'ERC721_APPROVAL';
  contractAddress: string;
  owner: string;
  approvedAddress: string | null;
  tokenId: string;
}

export interface EspaceOperatorApprovalChange {
  changeType: 'OPERATOR_APPROVAL';
  contractAddress: string;
  owner: string;
  operator: string;
  approved: boolean;
}

export interface EspaceErc1155TransferSingleChange {
  changeType: 'ERC1155_TRANSFER_SINGLE';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface EspaceErc1155TransferBatchChange {
  changeType: 'ERC1155_TRANSFER_BATCH';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  items: Erc1155TransferItem[];
}

export type EspaceChange =
  | EspaceNativeTransferChange
  | EspaceSelfDestructBurnChange
  | EspaceWrappedNativeDepositChange
  | EspaceWrappedNativeWithdrawalChange
  | EspaceErc20TransferChange
  | EspaceErc20ApprovalChange
  | EspaceErc721TransferChange
  | EspaceErc721ApprovalChange
  | EspaceOperatorApprovalChange
  | EspaceErc1155TransferSingleChange
  | EspaceErc1155TransferBatchChange;

interface NativeAsset extends FungibleAssetMetadata {
  assetType: 'NATIVE';
  rawAmount: string;
}

interface Erc20Asset extends FungibleAssetMetadata {
  assetType: 'ERC20';
  contractAddress: string;
  rawAmount: string;
}

interface Erc721Asset extends AssetMetadata {
  assetType: 'ERC721';
  contractAddress: string;
  tokenId: string;
}

interface Erc1155Asset {
  assetType: 'ERC1155';
  contractAddress: string;
  tokenId: string;
  rawAmount: string;
}

type TokenMovementAsset = Erc20Asset | Erc721Asset | Erc1155Asset;
type BurnAsset = NativeAsset | TokenMovementAsset;
type TransferAsset = NativeAsset | TokenMovementAsset;
type WithFields<TValue, TFields> = TValue extends unknown
  ? TValue & TFields
  : never;

export type TransferChange = WithFields<
  TransferAsset,
  {
    changeType: 'TRANSFER';
    from: string;
    to: string;
  }
>;

export type MintChange = WithFields<
  TokenMovementAsset,
  {
    changeType: 'MINT';
    to: string;
  }
>;

export type BurnChange = WithFields<
  BurnAsset,
  {
    changeType: 'BURN';
    from: string;
  }
>;

export interface AllowanceChange extends FungibleAssetMetadata {
  changeType: 'ALLOWANCE';
  assetType: 'ERC20';
  contractAddress: string;
  rawAmountBefore: string;
  rawAmountAfter: string;
  owner: string;
  spender: string;
}

export interface TokenApprovalChange extends AssetMetadata {
  changeType: 'TOKEN_APPROVAL';
  assetType: 'ERC721';
  contractAddress: string;
  tokenId: string;
  approvedAddressBefore: string | null;
  approvedAddressAfter: string | null;
}

export interface Erc721OperatorApprovalChange extends AssetMetadata {
  changeType: 'OPERATOR_APPROVAL';
  assetType: 'ERC721';
  contractAddress: string;
  owner: string;
  operator: string;
  approvedBefore: boolean;
  approvedAfter: boolean;
}

export interface Erc1155OperatorApprovalChange {
  changeType: 'OPERATOR_APPROVAL';
  assetType: 'ERC1155';
  contractAddress: string;
  owner: string;
  operator: string;
  approvedBefore: boolean;
  approvedAfter: boolean;
}

type CommonChange =
  | TransferChange
  | MintChange
  | BurnChange
  | AllowanceChange
  | TokenApprovalChange
  | Erc721OperatorApprovalChange
  | Erc1155OperatorApprovalChange;

export type HexChange = CommonChange;

export interface StakingDepositChange {
  changeType: 'STAKING_DEPOSIT';
  account: string;
  rawAmount: string;
}

export interface StakingWithdrawalChange {
  changeType: 'STAKING_WITHDRAWAL';
  account: string;
  rawAmount: string;
  rewardRawAmount: string;
}

export interface StakingBurnChange {
  changeType: 'STAKING_BURN';
  account: string;
  rawAmount: string;
}

export interface StakingVoteLockChange {
  changeType: 'STAKING_VOTE_LOCK';
  account: string;
  unlockBlockNumber: string;
  requiredLockedRawAmountBefore: string;
  requiredLockedRawAmountAfter: string;
}

export interface PosRegistrationChange {
  changeType: 'POS_REGISTRATION';
  account: string;
  posIdentifier: string;
  newlyLockedVoteCount: string;
  newlyLockedRawAmount: string;
}

export interface PosStakeIncreaseChange {
  changeType: 'POS_STAKE_INCREASE';
  account: string;
  posIdentifier: string;
  newlyLockedVoteCount: string;
  newlyLockedRawAmount: string;
}

export interface PosRetirementRequestChange {
  changeType: 'POS_RETIREMENT_REQUEST';
  account: string;
  posIdentifier: string;
  requestedVoteCount: string;
}

export type SponsoredResource = 'GAS' | 'STORAGE_COLLATERAL';

export interface SponsorshipDepositChange {
  changeType: 'SPONSORSHIP_DEPOSIT';
  sponsoredResource: SponsoredResource;
  sponsor: string;
  contractAddress: string;
  rawAmount: string;
}

export interface SponsorshipRefundChange {
  changeType: 'SPONSORSHIP_REFUND';
  sponsoredResource: SponsoredResource;
  sponsor: string;
  contractAddress: string;
  rawAmount: string;
}

export interface GasSponsorshipConfigurationChange {
  changeType: 'SPONSORSHIP_CONFIGURATION';
  sponsoredResource: 'GAS';
  contractAddress: string;
  sponsorBefore: string | null;
  sponsorAfter: string | null;
  maxSponsoredGasFeeRawAmountBefore: string;
  maxSponsoredGasFeeRawAmountAfter: string;
}

export interface StorageSponsorshipConfigurationChange {
  changeType: 'SPONSORSHIP_CONFIGURATION';
  sponsoredResource: 'STORAGE_COLLATERAL';
  contractAddress: string;
  sponsorBefore: string | null;
  sponsorAfter: string | null;
}

export type SponsorshipEligibilityTarget =
  | { type: 'ACCOUNT'; address: string }
  | { type: 'ALL_ACCOUNTS' };

export interface SponsorshipEligibilityRuleChange {
  changeType: 'SPONSORSHIP_ELIGIBILITY_RULE';
  contractAddress: string;
  appliesTo: SponsorshipEligibilityTarget;
  enabledBefore: boolean;
  enabledAfter: boolean;
}

export interface StoragePointConversionChange {
  changeType: 'STORAGE_POINT_CONVERSION';
  contractAddress: string;
  convertedCfxRawAmount: string;
}

export interface CrossSpaceEndpoint {
  space: 'CORE_SPACE' | 'ESPACE';
  address: string;
}

export interface CrossSpaceTransferChange {
  changeType: 'CROSS_SPACE_TRANSFER';
  from: CrossSpaceEndpoint;
  to: CrossSpaceEndpoint;
  rawAmount: string;
}

export type CoreChange =
  | CommonChange
  | StakingDepositChange
  | StakingWithdrawalChange
  | StakingBurnChange
  | StakingVoteLockChange
  | PosRegistrationChange
  | PosStakeIncreaseChange
  | PosRetirementRequestChange
  | SponsorshipDepositChange
  | SponsorshipRefundChange
  | GasSponsorshipConfigurationChange
  | StorageSponsorshipConfigurationChange
  | SponsorshipEligibilityRuleChange
  | StoragePointConversionChange
  | CrossSpaceTransferChange;

export type ExecutionStatus = 'SUCCESS' | 'FAILED' | 'NOT_EXECUTED';

export interface ExecutionFailure {
  code: string;
  message: string;
  reason?: string | null;
}

export interface SimulatedBlock {
  number: string;
  hash: string;
}

export interface EvmExecution {
  chainId: string;
  block: SimulatedBlock;
  status: ExecutionStatus;
  gasUsed: string;
  gasLimit: string;
  fee: string;
  burntFee: string;
  output: string;
  failure?: ExecutionFailure;
}

export interface EspaceExecution {
  chainId: string;
  block: SimulatedBlock;
  status: ExecutionStatus;
  gasUsed: string;
  gasLimit: string;
  gasCharged: string;
  fee: string;
  burntFee: string | null;
  output: string;
  failure: ExecutionFailure | null;
}

export interface CoreExecution {
  chainId: string;
  state: {
    epochNumber: string;
    pivotHash: string;
  };
  status: ExecutionStatus;
  gasUsed: string;
  gasLimit: string;
  gasCharged: string;
  fee: string;
  burntFee: string | null;
  gasCoveredBySponsor: boolean;
  storageCoveredBySponsor: boolean;
  output: string;
  failure: ExecutionFailure | null;
}

export interface EthereumResponse {
  execution: EvmExecution;
  changes: EvmChange[];
}

export interface EspaceResponse {
  execution: EspaceExecution;
  changes: EspaceChange[];
}

export interface CoreResponse {
  execution: CoreExecution;
  changes: CoreChange[];
}

export type RpcSimulationResponse =
  | EthereumResponse
  | EspaceResponse
  | CoreResponse;

export interface RpcErrorPayload {
  code: number;
  message: string;
  data?: unknown;
}

export interface RpcResultEnvelope {
  jsonrpc: '2.0';
  id: number;
  result: unknown;
}

export interface RpcErrorEnvelope {
  jsonrpc: '2.0';
  id: number;
  error: RpcErrorPayload;
}

export type RpcEnvelope = RpcResultEnvelope | RpcErrorEnvelope;
