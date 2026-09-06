use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_types::U64;
use conflux_provider::CoreAddress;
use conflux_simulation::espace::{
    AccessListItem, EspaceBlockContext, EspaceCompleteTransaction, EspaceExecutionOutcome,
    EspaceExecutionResult, EspaceLog, EspaceLogAddress, EspaceSimulation, EspaceSuccessOutput,
    EspaceTransactionCommon, SignedAuthorization,
};
use serde::Serialize;

use super::change::Change;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateEspaceTransactionResponse {
    state: State,
    transaction: CompletedTransaction,
    outcome: Outcome,
    changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct State {
    block_number: U64,
    block_hash: B256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum CompletedTransaction {
    #[serde(rename = "0x0")]
    Legacy {
        #[serde(flatten)]
        common: TransactionCommon,
        gas_price: U256,
    },
    #[serde(rename = "0x1")]
    Eip2930 {
        #[serde(flatten)]
        common: TransactionCommon,
        gas_price: U256,
        access_list: Vec<RpcAccessListItem>,
    },
    #[serde(rename = "0x2")]
    Eip1559 {
        #[serde(flatten)]
        common: TransactionCommon,
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<RpcAccessListItem>,
    },
    #[serde(rename = "0x4")]
    Eip7702 {
        #[serde(flatten)]
        common: TransactionCommon,
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<RpcAccessListItem>,
        authorization_list: Vec<RpcSignedAuthorization>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TransactionCommon {
    chain_id: U64,
    from: Address,
    to: Option<Address>,
    nonce: U64,
    gas: U64,
    value: U256,
    data: Bytes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RpcAccessListItem {
    address: Address,
    storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RpcSignedAuthorization {
    chain_id: U256,
    address: Address,
    nonce: U64,
    y_parity: U64,
    r: U256,
    s: U256,
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
            transaction: simulation.transaction.into(),
            outcome: simulation.execution.into(),
            changes: simulation.changes.into_iter().map(Into::into).collect(),
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

impl From<EspaceCompleteTransaction> for CompletedTransaction {
    fn from(transaction: EspaceCompleteTransaction) -> Self {
        match transaction {
            EspaceCompleteTransaction::Legacy { common, gas_price } => Self::Legacy {
                common: common.into(),
                gas_price,
            },
            EspaceCompleteTransaction::Eip2930 {
                common,
                gas_price,
                access_list,
            } => Self::Eip2930 {
                common: common.into(),
                gas_price,
                access_list: access_list.into_iter().map(Into::into).collect(),
            },
            EspaceCompleteTransaction::Eip1559 {
                common,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => Self::Eip1559 {
                common: common.into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list: access_list.into_iter().map(Into::into).collect(),
            },
            EspaceCompleteTransaction::Eip7702 {
                common,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
                authorization_list,
            } => Self::Eip7702 {
                common: common.into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list: access_list.into_iter().map(Into::into).collect(),
                authorization_list: authorization_list.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<EspaceTransactionCommon> for TransactionCommon {
    fn from(common: EspaceTransactionCommon) -> Self {
        Self {
            chain_id: common.chain_id.into(),
            from: common.from,
            to: common.to,
            nonce: common.nonce.into(),
            gas: common.gas_limit.into(),
            value: common.value,
            data: common.input,
        }
    }
}

impl From<AccessListItem> for RpcAccessListItem {
    fn from(item: AccessListItem) -> Self {
        Self {
            address: item.address,
            storage_keys: item.storage_keys,
        }
    }
}

impl From<SignedAuthorization> for RpcSignedAuthorization {
    fn from(authorization: SignedAuthorization) -> Self {
        let inner = authorization.inner();
        Self {
            chain_id: inner.chain_id,
            address: inner.address,
            nonce: inner.nonce.into(),
            y_parity: u64::from(authorization.y_parity()).into(),
            r: authorization.r(),
            s: authorization.s(),
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
