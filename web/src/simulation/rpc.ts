export type ExecutionSpace = 'evm' | 'espace' | 'core';

export interface Diagnostic {
  code: string;
  message: string;
  data?: Record<string, unknown>;
}

export type Changes<T> =
  | { status: 'complete'; items: T[] }
  | { status: 'notAnalyzed' }
  | { status: 'unavailable'; error: Diagnostic };

interface AssetMetadata {
  name?: string | null;
  symbol?: string | null;
}

interface FungibleAssetMetadata extends AssetMetadata {
  decimals?: number | null;
}

interface NativeCurrency {
  name: string;
  symbol: string;
  decimals: number;
}

export interface NativeTransferChange extends NativeCurrency {
  type: 'nativeTransfer';
  from: string;
  to: string;
  rawAmount: string;
}

export interface SelfDestructBurnChange extends NativeCurrency {
  type: 'selfDestructBurn';
  contractAddress: string;
  rawAmount: string;
}

export interface AccountDelegationChange {
  type: 'accountDelegation';
  account: string;
  before: DelegationState;
  after: DelegationState;
}

export interface DelegationState {
  delegate: string | null;
  nonce: string;
}

export interface WrappedNativeDepositChange
  extends FungibleAssetMetadata {
  type: 'wrappedNativeDeposit';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface WrappedNativeWithdrawalChange
  extends FungibleAssetMetadata {
  type: 'wrappedNativeWithdrawal';
  contractAddress: string;
  account: string;
  rawAmount: string;
}

export interface Erc20TransferChange extends FungibleAssetMetadata {
  type: 'erc20Transfer';
  contractAddress: string;
  from: string;
  to: string;
  rawAmount: string;
}

export interface Erc20MintChange extends FungibleAssetMetadata {
  type: 'erc20Mint';
  contractAddress: string;
  to: string;
  rawAmount: string;
}

export interface Erc20BurnChange extends FungibleAssetMetadata {
  type: 'erc20Burn';
  contractAddress: string;
  from: string;
  rawAmount: string;
}

export interface Erc20ApprovalChange extends FungibleAssetMetadata {
  type: 'erc20Approval';
  contractAddress: string;
  owner: string;
  spender: string;
  before: string;
  after: string;
}

export interface Erc721TransferChange extends AssetMetadata {
  type: 'erc721Transfer';
  contractAddress: string;
  from: string;
  to: string;
  tokenId: string;
}

export interface Erc721MintChange extends AssetMetadata {
  type: 'erc721Mint';
  contractAddress: string;
  to: string;
  tokenId: string;
}

export interface Erc721BurnChange extends AssetMetadata {
  type: 'erc721Burn';
  contractAddress: string;
  from: string;
  tokenId: string;
}

export interface Erc721ApprovalChange extends AssetMetadata {
  type: 'erc721Approval';
  contractAddress: string;
  owner: string;
  before: string | null;
  after: string | null;
  tokenId: string;
}

export interface OperatorApprovalChange {
  type: 'operatorApproval';
  contractAddress: string;
  owner: string;
  operator: string;
  before: boolean;
  after: boolean;
}

export interface Erc1155TransferSingleChange {
  type: 'erc1155TransferSingle';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface Erc1155MintSingleChange {
  type: 'erc1155MintSingle';
  contractAddress: string;
  operator: string;
  to: string;
  tokenId: string;
  rawAmount: string;
}

export interface Erc1155BurnSingleChange {
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

export interface Erc1155TransferBatchChange {
  type: 'erc1155TransferBatch';
  contractAddress: string;
  operator: string;
  from: string;
  to: string;
  items: Erc1155TransferItem[];
}

export interface Erc1155MintBatchChange {
  type: 'erc1155MintBatch';
  contractAddress: string;
  operator: string;
  to: string;
  items: Erc1155TransferItem[];
}

export interface Erc1155BurnBatchChange {
  type: 'erc1155BurnBatch';
  contractAddress: string;
  operator: string;
  from: string;
  items: Erc1155TransferItem[];
}

export type AssetChange = (
  | NativeTransferChange
  | SelfDestructBurnChange
  | AccountDelegationChange
  | WrappedNativeDepositChange
  | WrappedNativeWithdrawalChange
  | Erc20TransferChange
  | Erc20MintChange
  | Erc20BurnChange
  | Erc20ApprovalChange
  | Erc721TransferChange
  | Erc721MintChange
  | Erc721BurnChange
  | Erc721ApprovalChange
  | OperatorApprovalChange
  | Erc1155TransferSingleChange
  | Erc1155MintSingleChange
  | Erc1155BurnSingleChange
  | Erc1155TransferBatchChange
  | Erc1155MintBatchChange
  | Erc1155BurnBatchChange
) & { space?: ExecutionSpace };

export interface StakingDepositChange {
  type: 'stakingDeposit';
  account: string;
  rawAmount: string;
}

export interface StakingWithdrawalChange {
  type: 'stakingWithdrawal';
  account: string;
  principalRawAmount: string;
  rewardRawAmount: string;
}

export interface StakingVoteLockChange {
  type: 'stakingVoteLock';
  account: string;
  requiredLockedRawAmount: string;
  unlockBlockNumber: string;
}

export interface PosRegistrationChange {
  type: 'posRegistration';
  account: string;
  identifier: string;
  blsPublicKey: string;
  vrfPublicKey: string;
  initialVoteCount: string;
  lockedRawAmount: string;
}

export interface PosStakeIncreaseChange {
  type: 'posStakeIncrease';
  account: string;
  identifier: string;
  addedVoteCount: string;
  addedLockedRawAmount: string;
}

export interface PosRetirementRequestChange {
  type: 'posRetirementRequest';
  account: string;
  identifier: string;
  requestedVoteCount: string;
}

export type GovernanceParameter =
  | 'powBaseReward'
  | 'posRewardInterestRate'
  | 'storagePointProportion'
  | 'baseFeeShareProportion';

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
  type: 'governanceVoteCast';
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
  type: 'sponsorshipFunding';
  contractAddress: string;
  sponsor: string;
  contributedRawAmount: string;
  poolCreditedRawAmount: string;
}

export interface GasSponsorshipFundingChange
  extends SponsorshipFundingChange {
  resource: 'gas';
  gasFeeUpperBound: string;
  replacement: GasSponsorshipReplacement | null;
}

export interface StorageCollateralSponsorshipFundingChange
  extends SponsorshipFundingChange {
  resource: 'storageCollateral';
  replacement: StorageCollateralSponsorshipReplacement | null;
}

export type SponsorshipAccessRuleScope =
  | { type: 'account'; address: string }
  | { type: 'allAccounts' };

export interface ContractAdminSetChange {
  type: 'contractAdminSet';
  contractAddress: string;
  admin: string | null;
}

export interface SponsorshipAccessRuleSetChange {
  type: 'sponsorshipAccessRuleSet';
  contractAddress: string;
  scope: SponsorshipAccessRuleScope;
  enabled: boolean;
}

export interface StoragePointConversionChange {
  type: 'storagePointConversion';
  contractAddress: string;
  fromSponsorPoolRawAmount: string;
  fromStorageCollateralRawAmount: string;
}

export interface CrossSpaceEndpoint {
  space: 'coreSpace' | 'espace';
  address: string;
}

export interface CrossSpaceNativeTransferChange {
  type: 'crossSpaceNativeTransfer';
  from: CrossSpaceEndpoint;
  to: CrossSpaceEndpoint;
  rawAmount: string;
}

export interface GasSponsorshipChange {
  type: 'gasSponsorship';
  contractAddress: string;
  sponsor: string | null;
  balanceRawAmount: string;
  gasFeeUpperBoundRawAmount: string;
}

export interface StorageSponsorshipChange {
  type: 'storageSponsorship';
  contractAddress: string;
  sponsor: string | null;
  balanceRawAmount: string;
  storagePoints: { unused: string; used: string } | null;
}

export interface StorageCollateralChange {
  type: 'storageCollateral';
  contractAddress: string;
  rawAmount: string;
}

export interface ContractAdminChange {
  type: 'contractAdmin';
  contractAddress: string;
  state: { admin: string | null } | null;
}

export interface SponsorshipAccessRuleChange {
  type: 'sponsorshipAccessRule';
  contractAddress: string;
  scope: SponsorshipAccessRuleScope;
  enabled: boolean;
}

export type CoreProtocolChange = { space: 'core' } & (
  | StakingDepositChange
  | StakingWithdrawalChange
  | StakingVoteLockChange
  | PosRegistrationChange
  | PosStakeIncreaseChange
  | PosRetirementRequestChange
  | GovernanceVoteCastChange
  | GasSponsorshipChange
  | StorageSponsorshipChange
  | StorageCollateralChange
  | ContractAdminChange
  | SponsorshipAccessRuleChange
  | GasSponsorshipFundingChange
  | StorageCollateralSponsorshipFundingChange
  | ContractAdminSetChange
  | SponsorshipAccessRuleSetChange
  | StoragePointConversionChange
  | CrossSpaceNativeTransferChange
);

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
  error: Diagnostic;
}

export interface EvmFailedOutcome extends EvmExecutionAccounting {
  status: 'failed';
  error: Diagnostic;
}

export interface EvmRejectedOutcome {
  status: 'rejected';
  error: Diagnostic;
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

export interface EspaceState {
  blockNumber: string;
  blockHash: string;
}

export type EspaceCompletedTransaction =
  | EvmLegacyTransaction
  | EvmEip2930Transaction
  | EvmEip1559Transaction
  | EvmEip7702Transaction;

interface EspaceExecutionAccounting {
  gasUsed: string;
  gasFee: string;
  burntGasFee?: string;
}

export type EspaceOutcome =
  | (EspaceExecutionAccounting & {
      status: 'success';
      returnData: string;
      logs: SimulationLog[];
    })
  | (EspaceExecutionAccounting & {
      status: 'success';
      contractAddress: string;
      runtimeCode: string;
      logs: SimulationLog[];
    })
  | (EspaceExecutionAccounting & {
      status: 'reverted';
      revertData: string;
      error: Diagnostic;
    })
  | (EspaceExecutionAccounting & {
      status: 'failed';
      error: Diagnostic;
    })
  | { status: 'rejected'; error: Diagnostic };

export interface CoreState {
  epochNumber: string;
  pivotHash: string;
}

export interface CoreAccessListItem {
  address: string;
  storageKeys: string[];
}

interface CoreCompletedTransactionBase {
  type: string;
  chainId: string;
  from: string;
  to: string | null;
  nonce: string;
  gas: string;
  value: string;
  data: string;
  storageLimit: string;
  epochHeight: string;
}

export interface CoreCip155Transaction
  extends CoreCompletedTransactionBase {
  type: '0x0';
  gasPrice: string;
}

export interface CoreCip2930Transaction
  extends CoreCompletedTransactionBase {
  type: '0x1';
  gasPrice: string;
  accessList: CoreAccessListItem[];
}

export interface CoreCip1559Transaction
  extends CoreCompletedTransactionBase {
  type: '0x2';
  maxFeePerGas: string;
  maxPriorityFeePerGas: string;
  accessList: CoreAccessListItem[];
}

export type CoreCompletedTransaction =
  | CoreCip155Transaction
  | CoreCip2930Transaction
  | CoreCip1559Transaction;

interface CoreExecutionAccounting {
  gasUsed: string;
  gasFee: string;
  burntGasFee?: string;
  effectiveGasPrice: string;
  gasCoveredBySponsor: boolean;
  storageCollateralized: string;
  storageCoveredBySponsor: boolean;
}

export type CoreOutcome =
  | (CoreExecutionAccounting & {
      status: 'success';
      returnData: string;
      logs: SimulationLog[];
    })
  | (CoreExecutionAccounting & {
      status: 'success';
      contractAddress: string;
      runtimeCode: string;
      logs: SimulationLog[];
    })
  | (CoreExecutionAccounting & {
      status: 'reverted';
      revertData: string;
      error: Diagnostic;
    })
  | (CoreExecutionAccounting & {
      status: 'failed';
      error: Diagnostic;
    })
  | { status: 'rejected'; error: Diagnostic };

interface CommonPartialTransaction extends Partial<EvmCompletedTransactionBase> {
  from: string;
  gasPrice?: string;
  maxFeePerGas?: string;
  maxPriorityFeePerGas?: string;
  accessList?: EvmAccessListItem[];
}

export interface EvmPartialTransaction extends CommonPartialTransaction {
  maxFeePerBlobGas?: string;
  blobVersionedHashes?: string[];
  authorizationList?: EvmSignedAuthorization[];
}

export interface CorePartialTransaction extends CommonPartialTransaction {
  storageLimit?: string;
  epochHeight?: string;
}

export type TransactionInput<Complete, Partial> =
  | { status: 'complete'; fields: Complete }
  | { status: 'partial'; fields: Partial };

export interface EthereumResponse {
  state: EvmState | null;
  transaction: TransactionInput<EvmCompletedTransaction, EvmPartialTransaction>;
  outcome: EvmOutcome;
  changes: Changes<AssetChange>;
}

export interface EspaceResponse {
  state: EspaceState | null;
  transaction: TransactionInput<EspaceCompletedTransaction, EvmPartialTransaction>;
  outcome: EspaceOutcome;
  changes: Changes<AssetChange>;
}

export interface CoreResponse {
  state: CoreState | null;
  transaction: TransactionInput<CoreCompletedTransaction, CorePartialTransaction>;
  outcome: CoreOutcome;
  changes: Changes<AssetChange | CoreProtocolChange>;
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
