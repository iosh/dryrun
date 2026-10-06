export interface RpcErrorPayload {
  code: number;
  message: string;
  data?: unknown;
}

export type RpcEnvelope = {
  jsonrpc: '2.0';
  id: string | number | null;
} & ({ result: unknown } | { error: RpcErrorPayload });

export interface Diagnostic {
  code: string;
  message: string;
}

// Preserve the completed upstream request, including fields the form does not expose.
export interface CompletedTransaction {
  from: string;
  to?: string | null;
  chainId: string;
  [key: string]: unknown;
}

export interface Fee {
  gasPrice: string;
  baseFee: string;
  amount: string;
  blobGasPrice?: string;
  payer?:
    | { type: 'sender'; address: string }
    | { type: 'sponsor'; contract: string };
}

interface Execution {
  gasUsed: string;
  gasCharged?: string;
  fee: Fee;
}

export type Outcome =
  | { status: 'rejected'; reason: string; message: string }
  | (Execution & {
    status: 'success';
    output: string;
    changes: ChangeSet | { error: Diagnostic };
  })
  | (Execution & { status: 'reverted'; output: string; reason?: string })
  | (Execution & { status: 'halted'; reason: string });

export type RpcSimulationResponse = {
  transaction: CompletedTransaction;
  outcome: Outcome;
} & (
  | { block: { number: string; hash: string } }
  | {
    epoch: { number: string; pivotHash: string };
    approximate: true;
    limitations: string[];
  }
);

export type Asset =
  | { type: 'native' }
  | { type: 'erc20'; token: string }
  | { type: 'erc721' | 'erc1155'; token: string; id: string };

export interface BalanceChange {
  holder: string;
  asset: Asset;
  before: string;
  after: string;
}

export type ApprovalChange =
  | {
    type: 'erc20';
    token: string;
    owner: string;
    spender: string;
    before: string;
    after: string;
  }
  | {
    type: 'erc721';
    token: string;
    id: string;
    before: string | null;
    after: string | null;
  }
  | {
    type: 'operator';
    token: string;
    owner: string;
    operator: string;
    before: boolean;
    after: boolean;
  };

export interface DelegationChange {
  account: string;
  before: string | null;
  after: string | null;
}

export interface TokenMetadata {
  name?: string;
  symbol?: string;
  decimals?: number;
}

export interface InvolvedContract {
  address: string;
  called: boolean;
  created: boolean;
  destroyed: boolean;
  storageModified: boolean;
}

export interface ChangeSet {
  balances: BalanceChange[];
  approvals: ApprovalChange[];
  delegations: DelegationChange[];
  tokens: Record<string, TokenMetadata>;
  contracts: InvolvedContract[];
  tokenReadFailures: { token: string; function: string }[];
  protocol?: ProtocolChange[];
}

export interface Deposit {
  amount: string;
  depositTime: string;
  accumulatedInterestRate: string;
}

export interface VoteStake {
  amount: string;
  unlockBlockNumber: string;
}

export interface PosStake {
  account: string | null;
  registered: string;
  unlocked: string;
}

export interface GovernanceVote {
  index: string;
  votes: [string, string, string];
}

export interface GasSponsor {
  sponsor: string | null;
  balance: string;
  gasBound: string;
}

export interface StorageSponsor {
  sponsor: string | null;
  balance: string;
  storagePoints: { unused: string; used: string } | null;
}

export type ProtocolChange =
  | {
    type: 'stakingBalance' | 'accumulatedInterestReturn' | 'collateralForStorage';
    account: string;
    before: string;
    after: string;
  }
  | { type: 'depositList'; account: string; before: Deposit[]; after: Deposit[] }
  | { type: 'voteStakeList'; account: string; before: VoteStake[]; after: VoteStake[] }
  | { type: 'posIdentifier'; account: string; before: string | null; after: string | null }
  | { type: 'posStake'; identifier: string; before: PosStake; after: PosStake }
  | { type: 'governanceVotes'; voter: string; before: GovernanceVote[]; after: GovernanceVote[] }
  | { type: 'admin'; contract: string; before: string | null; after: string | null }
  | { type: 'gasSponsor'; contract: string; before: GasSponsor; after: GasSponsor }
  | { type: 'storageSponsor'; contract: string; before: StorageSponsor; after: StorageSponsor }
  | {
    type: 'sponsorWhitelist';
    contract: string;
    user: string | null;
    before: boolean;
    after: boolean;
  };
