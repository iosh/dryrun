use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_types::U64;
use conflux_provider::CoreAddress;
use conflux_simulation::espace::{
    EspaceAccountDelegation, EspaceAccountDelegationChange, EspaceBlockContext,
    EspaceExecutionOutcome, EspaceExecutionResult, EspaceLog, EspaceLogAddress,
    EspaceNativeTransferChange, EspaceSelfDestructBurnChange, EspaceSimulation,
    EspaceStandardChange, EspaceStateChange, EspaceSuccessOutput, EspaceTypedTransaction,
    EspaceWrappedNativeDepositChange, EspaceWrappedNativeWithdrawalChange,
};
use serde::Serialize;

use super::change::{
    Erc20Metadata as RpcErc20Metadata, Erc721CollectionMetadata as RpcErc721CollectionMetadata,
    Erc1155TransferItem as RpcErc1155TransferItem, NativeCurrency,
};

mod u256_hex {
    use alloy_primitives::U256;
    use serde::{Serialize, Serializer};

    pub(super) fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateEspaceTransactionResponse {
    state: State,
    transaction: EspaceTypedTransaction,
    outcome: Outcome,
    changes: Changes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
enum Changes {
    Complete { items: Vec<StandaloneChange> },
    Unavailable { error: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StandaloneChange {
    NativeTransfer {
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    AccountDelegation {
        account: Address,
        before: DelegationState,
        after: DelegationState,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc20Mint {
        contract_address: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc20Burn {
        contract_address: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc20Approval {
        contract_address: Address,
        owner: Address,
        spender: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        before: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        after: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    Erc721Mint {
        contract_address: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    Erc721Burn {
        contract_address: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155MintSingle {
        contract_address: Address,
        operator: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155BurnSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<RpcErc1155TransferItem>,
    },
    Erc1155MintBatch {
        contract_address: Address,
        operator: Address,
        to: Address,
        items: Vec<RpcErc1155TransferItem>,
    },
    Erc1155BurnBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        items: Vec<RpcErc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DelegationState {
    delegate: Option<Address>,
    nonce: U64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct State {
    block_number: U64,
    block_hash: B256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "status",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
enum Outcome {
    Success {
        #[serde(flatten)]
        accounting: ExecutionAccounting,
        #[serde(flatten)]
        output: SuccessOutput,
        logs: Vec<SimulationLog>,
    },
    Reverted {
        #[serde(flatten)]
        accounting: ExecutionAccounting,
        revert_data: Bytes,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Failed {
        #[serde(flatten)]
        accounting: ExecutionAccounting,
        error: String,
    },
    Rejected {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExecutionAccounting {
    gas_used: U64,
    gas_fee: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    burnt_gas_fee: Option<U256>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged, rename_all_fields = "camelCase")]
enum SuccessOutput {
    Call {
        return_data: Bytes,
    },
    Create {
        contract_address: Address,
        runtime_code: Bytes,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SimulationLog {
    address: LogAddress,
    topics: Vec<B256>,
    data: Bytes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
enum LogAddress {
    Espace(Address),
    CoreSpace(CoreAddress),
}

impl From<EspaceSimulation> for SimulateEspaceTransactionResponse {
    fn from(simulation: EspaceSimulation) -> Self {
        Self {
            state: simulation.context.into(),
            transaction: simulation.transaction,
            outcome: simulation.execution.into(),
            changes: simulation.changes.into(),
        }
    }
}

impl From<conflux_simulation::espace::EspaceChanges> for Changes {
    fn from(changes: conflux_simulation::espace::EspaceChanges) -> Self {
        match changes {
            conflux_simulation::espace::EspaceChanges::Complete(changes) => Self::Complete {
                items: changes.into_items().into_iter().map(Into::into).collect(),
            },
            conflux_simulation::espace::EspaceChanges::Unavailable(error) => Self::Unavailable {
                error: error.to_string(),
            },
        }
    }
}

impl From<EspaceStateChange> for StandaloneChange {
    fn from(change: EspaceStateChange) -> Self {
        match change {
            EspaceStateChange::NativeTransfer(change) => change.into(),
            EspaceStateChange::SelfDestructBurn(change) => change.into(),
            EspaceStateChange::WrappedNativeDeposit(change) => change.into(),
            EspaceStateChange::WrappedNativeWithdrawal(change) => change.into(),
            EspaceStateChange::AccountDelegation(change) => change.into(),
            EspaceStateChange::Standard(change) => change.into(),
        }
    }
}

impl From<EspaceNativeTransferChange> for StandaloneChange {
    fn from(change: EspaceNativeTransferChange) -> Self {
        Self::NativeTransfer {
            from: change.from,
            to: change.to,
            raw_amount: change.raw_amount,
            currency: change.currency.into(),
        }
    }
}

impl From<EspaceSelfDestructBurnChange> for StandaloneChange {
    fn from(change: EspaceSelfDestructBurnChange) -> Self {
        Self::SelfDestructBurn {
            contract_address: change.contract_address,
            raw_amount: change.raw_amount,
            currency: change.currency.into(),
        }
    }
}

impl From<EspaceWrappedNativeDepositChange> for StandaloneChange {
    fn from(change: EspaceWrappedNativeDepositChange) -> Self {
        Self::WrappedNativeDeposit {
            contract_address: change.contract_address,
            account: change.account,
            raw_amount: change.raw_amount,
            metadata: change.metadata.into(),
        }
    }
}

impl From<EspaceWrappedNativeWithdrawalChange> for StandaloneChange {
    fn from(change: EspaceWrappedNativeWithdrawalChange) -> Self {
        Self::WrappedNativeWithdrawal {
            contract_address: change.contract_address,
            account: change.account,
            raw_amount: change.raw_amount,
            metadata: change.metadata.into(),
        }
    }
}

impl From<EspaceAccountDelegationChange> for StandaloneChange {
    fn from(change: EspaceAccountDelegationChange) -> Self {
        Self::AccountDelegation {
            account: change.account,
            before: change.before.into(),
            after: change.after.into(),
        }
    }
}

impl From<EspaceAccountDelegation> for DelegationState {
    fn from(state: EspaceAccountDelegation) -> Self {
        Self {
            delegate: state.delegate,
            nonce: state.nonce.into(),
        }
    }
}

impl From<EspaceStandardChange> for StandaloneChange {
    fn from(change: EspaceStandardChange) -> Self {
        match change {
            EspaceStandardChange::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata,
            } => Self::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata,
            } => Self::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
                metadata,
            } => Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata,
            } => Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata,
            } => Self::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata,
            } => Self::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata: metadata.into(),
            },
            EspaceStandardChange::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
                metadata,
            } => Self::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
                metadata: metadata.into(),
            },
            EspaceStandardChange::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            } => Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            },
            EspaceStandardChange::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            },
            EspaceStandardChange::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            },
            EspaceStandardChange::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            } => Self::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            },
            EspaceStandardChange::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => Self::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items: items.into_iter().map(Into::into).collect(),
            },
            EspaceStandardChange::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items,
            } => Self::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items: items.into_iter().map(Into::into).collect(),
            },
            EspaceStandardChange::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items,
            } => Self::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items: items.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<EspaceBlockContext> for State {
    fn from(context: EspaceBlockContext) -> Self {
        Self {
            block_number: context.number.into(),
            block_hash: context.hash,
        }
    }
}

impl From<EspaceExecutionOutcome> for Outcome {
    fn from(outcome: EspaceExecutionOutcome) -> Self {
        match outcome {
            EspaceExecutionOutcome::Success {
                result,
                output,
                logs,
            } => Self::Success {
                accounting: result.into(),
                output: match output {
                    EspaceSuccessOutput::Call { return_data } => {
                        SuccessOutput::Call { return_data }
                    }
                    EspaceSuccessOutput::Create {
                        address,
                        runtime_code,
                    } => SuccessOutput::Create {
                        contract_address: address,
                        runtime_code,
                    },
                },
                logs: logs.into_iter().map(Into::into).collect(),
            },
            EspaceExecutionOutcome::Reverted {
                result,
                revert_data,
                reason,
            } => Self::Reverted {
                accounting: result.into(),
                revert_data,
                reason: reason.map(|reason| reason.to_string()),
            },
            EspaceExecutionOutcome::Failed { result, failure } => Self::Failed {
                accounting: result.into(),
                error: failure.to_string(),
            },
            EspaceExecutionOutcome::NotExecuted(rejection) => Self::Rejected {
                error: rejection.to_string(),
            },
        }
    }
}

impl From<EspaceExecutionResult> for ExecutionAccounting {
    fn from(result: EspaceExecutionResult) -> Self {
        Self {
            gas_used: result.gas().gas_used().into(),
            gas_fee: result.fee().charged_amount(),
            burnt_gas_fee: result.fee().burnt_amount(),
        }
    }
}

impl From<EspaceLog> for SimulationLog {
    fn from(log: EspaceLog) -> Self {
        Self {
            address: match log.address {
                EspaceLogAddress::Espace(address) => LogAddress::Espace(address),
                EspaceLogAddress::CoreSpace(address) => LogAddress::CoreSpace(address),
            },
            topics: log.topics,
            data: log.data,
        }
    }
}
