interface AssetMetadata {
  name?: string;
  symbol?: string;
}

interface FungibleAssetMetadata extends AssetMetadata {
  decimals?: number;
}

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
  changes: HexChange[];
}

export interface EspaceResponse {
  execution: EspaceExecution;
  changes: HexChange[];
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
