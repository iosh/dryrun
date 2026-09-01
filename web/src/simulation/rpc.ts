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
  type: 'nativeTransfer';
  from: string;
  to: string;
  rawAmount: string;
}

export interface EvmSelfDestructBurnChange extends EvmNativeCurrency {
  type: 'selfDestructBurn';
  contractAddress: string;
  rawAmount: string;
}

export interface EvmAccountDelegationChange {
  type: 'accountDelegation';
  account: string;
  before: EvmDelegationState;
  after: EvmDelegationState;
}

export interface EvmDelegationState {
  delegate: string | null;
  nonce: string;
}

export interface EvmWrappedNativeDepositChange
  extends FungibleAssetMetadata {
  type: 'wrappedNativeDeposit';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface EvmWrappedNativeWithdrawalChange
  extends FungibleAssetMetadata {
  type: 'wrappedNativeWithdrawal';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface EvmErc20TransferChange extends FungibleAssetMetadata {
  type: 'erc20Transfer';
  contractAddress: string;
  from: string;
  to: string;
  rawAmount: string;
}

export interface EvmErc20MintChange extends FungibleAssetMetadata {
  type: 'erc20Mint';
  contractAddress: string;
  to: string;
  rawAmount: string;
}

export interface EvmErc20BurnChange extends FungibleAssetMetadata {
  type: 'erc20Burn';
  contractAddress: string;
  from: string;
  rawAmount: string;
}

export interface EvmErc20ApprovalChange extends FungibleAssetMetadata {
  type: 'erc20Approval';
  contractAddress: string;
  owner: string;
  spender: string;
  before: string;
  after: string;
}

export interface EvmErc721TransferChange extends AssetMetadata {
  type: 'erc721Transfer';
  contractAddress: string;
  from: string;
  to: string;
  tokenId: string;
}

export interface EvmErc721MintChange extends AssetMetadata {
  type: 'erc721Mint';
  contractAddress: string;
  to: string;
  tokenId: string;
}

export interface EvmErc721BurnChange extends AssetMetadata {
  type: 'erc721Burn';
  contractAddress: string;
  from: string;
  tokenId: string;
}

export interface EvmErc721ApprovalChange extends AssetMetadata {
  type: 'erc721Approval';
  contractAddress: string;
  owner: string;
  before: string | null;
  after: string | null;
  tokenId: string;
}

export interface EvmOperatorApprovalChange {
  type: 'operatorApproval';
  contractAddress: string;
  owner: string;
  operator: string;
  before: boolean;
  after: boolean;
}

export interface EvmErc1155TransferSingleChange {
  type: 'erc1155TransferSingle';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface EvmErc1155MintSingleChange {
  type: 'erc1155MintSingle';
  contractAddress: string;
  operator: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface EvmErc1155BurnSingleChange {
  type: 'erc1155BurnSingle';
  contractAddress: string;
  operator: string;
  from: string;
  tokenId: string;
  rawAmount: string;
}

export interface Erc1155TransferItem {
  tokenId: string;
  rawAmount: string;
}

export interface EvmErc1155TransferBatchChange {
  type: 'erc1155TransferBatch';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  items: Erc1155TransferItem[];
}

export interface EvmErc1155MintBatchChange {
  type: 'erc1155MintBatch';
  contractAddress: string;
  operator: string;
  to: string;
  items: Erc1155TransferItem[];
}

export interface EvmErc1155BurnBatchChange {
  type: 'erc1155BurnBatch';
  contractAddress: string;
  operator: string;
  from: string;
  items: Erc1155TransferItem[];
}

export type EvmChange =
  | EvmNativeTransferChange
  | EvmSelfDestructBurnChange
  | EvmAccountDelegationChange
  | EvmWrappedNativeDepositChange
  | EvmWrappedNativeWithdrawalChange
  | EvmErc20TransferChange
  | EvmErc20MintChange
  | EvmErc20BurnChange
  | EvmErc20ApprovalChange
  | EvmErc721TransferChange
  | EvmErc721MintChange
  | EvmErc721BurnChange
  | EvmErc721ApprovalChange
  | EvmOperatorApprovalChange
  | EvmErc1155TransferSingleChange
  | EvmErc1155MintSingleChange
  | EvmErc1155BurnSingleChange
  | EvmErc1155TransferBatchChange
  | EvmErc1155MintBatchChange
  | EvmErc1155BurnBatchChange;

export type EvmChanges =
  | { status: 'complete'; items: EvmChange[] }
  | { status: 'unavailable'; error: string };

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

export interface GasSponsorshipReplacement {
  previousSponsor: string;
  poolRefundedRawAmount: string;
}

export interface StorageCollateralSponsorshipReplacement {
  previousSponsor: string;
  poolRefundedRawAmount: string;
  collateralCompensationRawAmount: string;
}

interface SponsorshipFundingChange {
  changeType: 'SPONSORSHIP_FUNDING';
  contractAddress: string;
  sponsor: string;
  contributedRawAmount: string;
  poolCreditedRawAmount: string;
}

export interface GasSponsorshipFundingChange
  extends SponsorshipFundingChange {
  sponsoredResource: 'GAS';
  gasFeeUpperBoundRawAmount: string;
  replacement: GasSponsorshipReplacement | null;
}

export interface StorageCollateralSponsorshipFundingChange
  extends SponsorshipFundingChange {
  sponsoredResource: 'STORAGE_COLLATERAL';
  replacement: StorageCollateralSponsorshipReplacement | null;
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
  fromSponsorPoolRawAmount: string;
  fromStorageCollateralRawAmount: string;
}

export interface CrossSpaceEndpoint {
  space: 'CORE_SPACE' | 'ESPACE';
  address: string;
}

export interface CrossSpaceNativeTransferChange {
  changeType: 'CROSS_SPACE_NATIVE_TRANSFER';
  from: CrossSpaceEndpoint;
  to: CrossSpaceEndpoint;
  rawAmount: string;
}

export interface NestedEspaceChange {
  changeType: 'ESPACE';
  change: EspaceChange;
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
  | GasSponsorshipFundingChange
  | StorageCollateralSponsorshipFundingChange
  | ContractAdminSetChange
  | SponsorshipAccessRuleSetChange
  | StoragePointConversionChange
  | CrossSpaceNativeTransferChange
  | NestedEspaceChange;

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

export interface EvmState {
  blockNumber: string;
  blockHash: string;
}

export interface EvmAccessListItem {
  address: string;
  storageKeys: string[];
}

export interface EvmSignedAuthorization {
  chainId: string;
  address: string;
  nonce: string;
  yParity: string;
  r: string;
  s: string;
}

interface EvmCompletedTransactionBase {
  type: string;
  chainId: string;
  from: string;
  to: string | null;
  nonce: string;
  gas: string;
  value: string;
  data: string;
}

export interface EvmLegacyTransaction extends EvmCompletedTransactionBase {
  type: '0x0';
  gasPrice: string;
}

export interface EvmEip2930Transaction extends EvmCompletedTransactionBase {
  type: '0x1';
  gasPrice: string;
  accessList: EvmAccessListItem[];
}

export interface EvmEip1559Transaction extends EvmCompletedTransactionBase {
  type: '0x2';
  maxFeePerGas: string;
  maxPriorityFeePerGas: string;
  accessList: EvmAccessListItem[];
}

export interface EvmEip4844Transaction extends EvmCompletedTransactionBase {
  type: '0x3';
  maxFeePerGas: string;
  maxPriorityFeePerGas: string;
  maxFeePerBlobGas: string;
  accessList: EvmAccessListItem[];
  blobVersionedHashes: string[];
}

export interface EvmEip7702Transaction extends EvmCompletedTransactionBase {
  type: '0x4';
  maxFeePerGas: string;
  maxPriorityFeePerGas: string;
  accessList: EvmAccessListItem[];
  authorizationList: EvmSignedAuthorization[];
}

export type EvmCompletedTransaction =
  | EvmLegacyTransaction
  | EvmEip2930Transaction
  | EvmEip1559Transaction
  | EvmEip4844Transaction
  | EvmEip7702Transaction;

interface EvmExecutionAccounting {
  gasUsed: string;
  effectiveGasPrice: string;
  gasFee: string;
  burntGasFee?: string;
  blobGasUsed?: string;
  blobGasPrice?: string;
  blobGasFee?: string;
}

export interface EvmSuccessCallOutcome extends EvmExecutionAccounting {
  status: 'success';
  returnData: string;
  logs: SimulationLog[];
}

export interface EvmSuccessCreateOutcome extends EvmExecutionAccounting {
  status: 'success';
  contractAddress: string;
  runtimeCode: string;
  logs: SimulationLog[];
}

export interface EvmRevertedOutcome extends EvmExecutionAccounting {
  status: 'reverted';
  revertData: string;
  reason?: string;
}

export interface EvmFailedOutcome extends EvmExecutionAccounting {
  status: 'failed';
  error: string;
}

export interface EvmRejectedOutcome {
  status: 'rejected';
  error: string;
}

export type EvmOutcome =
  | EvmSuccessCallOutcome
  | EvmSuccessCreateOutcome
  | EvmRevertedOutcome
  | EvmFailedOutcome
  | EvmRejectedOutcome;

export interface SimulationLog {
  address: string;
  topics: string[];
  data: string;
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
  state: EvmState;
  transaction: EvmCompletedTransaction;
  outcome: EvmOutcome;
  changes: EvmChanges;
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
