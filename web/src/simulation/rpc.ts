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

export interface Erc20TransferChange extends FungibleAssetMetadata {
  changeType: 'ERC20_TRANSFER';
  contractAddress: string;
  from: string;
  to: string;
  rawAmount: string;
}

export interface Erc20ApprovalChange extends FungibleAssetMetadata {
  changeType: 'ERC20_APPROVAL';
  contractAddress: string;
  owner: string;
  spender: string;
  approvedAmount: string;
}

export interface Erc721TransferChange extends AssetMetadata {
  changeType: 'ERC721_TRANSFER';
  contractAddress: string;
  from: string;
  to: string;
  tokenId: string;
}

export interface Erc721ApprovalChange extends AssetMetadata {
  changeType: 'ERC721_APPROVAL';
  contractAddress: string;
  owner: string;
  approvedAddress: string | null;
  tokenId: string;
}

export interface OperatorApprovalChange {
  changeType: 'OPERATOR_APPROVAL';
  contractAddress: string;
  owner: string;
  operator: string;
  approved: boolean;
}

export interface Erc1155TransferSingleChange {
  changeType: 'ERC1155_TRANSFER_SINGLE';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface Erc1155TransferBatchChange {
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
  | Erc20TransferChange
  | Erc20ApprovalChange
  | Erc721TransferChange
  | Erc721ApprovalChange
  | OperatorApprovalChange
  | Erc1155TransferSingleChange
  | Erc1155TransferBatchChange;

interface CoreNativeCurrency {
  name: string;
  symbol: string;
  decimals: number;
}

export interface CoreNativeTransferChange extends CoreNativeCurrency {
  changeType: 'NATIVE_TRANSFER';
  from: string;
  to: string;
  rawAmount: string;
}

export interface CoreNativeBurnChange extends CoreNativeCurrency {
  changeType: 'NATIVE_BURN';
  from: string;
  rawAmount: string;
}

export interface StakingDepositChange {
  changeType: 'STAKING_DEPOSIT';
  account: string;
  rawAmount: string;
}

export interface StakingWithdrawalChange {
  changeType: 'STAKING_WITHDRAWAL';
  account: string;
  principalRawAmount: string;
  rewardRawAmount: string;
}

export interface StakingVoteLockChange {
  changeType: 'STAKING_VOTE_LOCK';
  account: string;
  requiredLockedRawAmount: string;
  unlockBlockNumber: string;
}

export interface PosRegistrationChange {
  changeType: 'POS_REGISTRATION';
  account: string;
  identifier: string;
  blsPublicKey: string;
  vrfPublicKey: string;
  initialVoteCount: string;
  lockedRawAmount: string;
}

export interface PosStakeIncreaseChange {
  changeType: 'POS_STAKE_INCREASE';
  account: string;
  identifier: string;
  addedVoteCount: string;
  addedLockedRawAmount: string;
}

export interface PosRetirementRequestChange {
  changeType: 'POS_RETIREMENT_REQUEST';
  account: string;
  identifier: string;
  requestedVoteCount: string;
}

export type GovernanceParameter =
  | 'POW_BASE_REWARD'
  | 'POS_REWARD_INTEREST_RATE'
  | 'STORAGE_POINT_PROPORTION'
  | 'BASE_FEE_SHARE_PROPORTION';

export interface VoteAllocation {
  unchanged: string;
  increase: string;
  decrease: string;
}

export interface GovernanceVote {
  parameter: GovernanceParameter;
  allocation: VoteAllocation;
  replacedAllocation: VoteAllocation | null;
}

export interface GovernanceVoteCastChange {
  changeType: 'GOVERNANCE_VOTE_CAST';
  voter: string;
  round: string;
  votes: GovernanceVote[];
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

export type SponsorshipAccessRuleScope =
  | { type: 'ACCOUNT'; address: string }
  | { type: 'ALL_ACCOUNTS' };

export interface ContractAdminSetChange {
  changeType: 'CONTRACT_ADMIN_SET';
  contractAddress: string;
  admin: string | null;
}

export interface SponsorshipAccessRuleSetChange {
  changeType: 'SPONSORSHIP_ACCESS_RULE_SET';
  contractAddress: string;
  scope: SponsorshipAccessRuleScope;
  enabled: boolean;
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
  | CoreNativeTransferChange
  | CoreNativeBurnChange
  | Erc20TransferChange
  | Erc20ApprovalChange
  | Erc721TransferChange
  | Erc721ApprovalChange
  | OperatorApprovalChange
  | Erc1155TransferSingleChange
  | Erc1155TransferBatchChange
  | StakingDepositChange
  | StakingWithdrawalChange
  | StakingVoteLockChange
  | PosRegistrationChange
  | PosStakeIncreaseChange
  | PosRetirementRequestChange
  | GovernanceVoteCastChange
  | SponsorshipDepositChange
  | SponsorshipRefundChange
  | GasSponsorshipConfigurationChange
  | StorageSponsorshipConfigurationChange
  | ContractAdminSetChange
  | SponsorshipAccessRuleSetChange
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
