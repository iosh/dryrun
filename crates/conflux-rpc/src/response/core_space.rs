use alloy_primitives::Address as EspaceAddress;
use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_provider::CoreAddress;
use conflux_simulation::core_space as simulation_core_space;
use serde::Serialize;

use super::{b256_to_wire, core_space_change, u256_to_wire};

#[derive(Debug, thiserror::Error)]
#[error("failed to encode `{field}` as a Core Space address: {message}")]
pub(crate) struct ResponseMappingError {
    field: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateCoreSpaceTransactionResponse {
    state: State,
    transaction: CompletedTransaction,
    outcome: Outcome,
    changes: Changes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct State {
    epoch_number: U64,
    pivot_hash: H256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum CompletedTransaction {
    #[serde(rename = "0x0")]
    Cip155 {
        #[serde(flatten)]
        common: TransactionCommon,
        gas_price: U256,
    },
    #[serde(rename = "0x1")]
    Cip2930 {
        #[serde(flatten)]
        common: TransactionCommon,
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    #[serde(rename = "0x2")]
    Cip1559 {
        #[serde(flatten)]
        common: TransactionCommon,
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TransactionCommon {
    chain_id: U64,
    from: RpcAddress,
    to: Option<RpcAddress>,
    nonce: U256,
    gas: U256,
    value: U256,
    data: CoreSpaceRpcBytes,
    storage_limit: U64,
    epoch_height: U64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct AccessListItem {
    address: RpcAddress,
    storage_keys: Vec<H256>,
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
        revert_data: CoreSpaceRpcBytes,
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
    effective_gas_price: U256,
    gas_covered_by_sponsor: bool,
    storage_collateralized: U64,
    storage_covered_by_sponsor: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged, rename_all_fields = "camelCase")]
enum SuccessOutput {
    Call {
        return_data: CoreSpaceRpcBytes,
    },
    Create {
        contract_address: RpcAddress,
        runtime_code: CoreSpaceRpcBytes,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SimulationLog {
    address: LogAddress,
    topics: Vec<H256>,
    data: CoreSpaceRpcBytes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
enum LogAddress {
    CoreSpace(RpcAddress),
    Espace(EspaceAddress),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
enum Changes {
    Complete {
        items: Vec<core_space_change::Change>,
    },
    Unavailable {
        error: String,
    },
}

impl SimulateCoreSpaceTransactionResponse {
    pub(crate) fn try_from_simulation(
        simulation: simulation_core_space::CoreSpaceSimulation,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        let (context, transaction, outcome, changes) = simulation.into_parts();
        Ok(Self {
            state: context.into(),
            transaction: CompletedTransaction::try_from_simulation(transaction, network)?,
            outcome: Outcome::try_from_simulation(outcome, network)?,
            changes: Changes::try_from_simulation(changes, network)?,
        })
    }
}

impl From<simulation_core_space::CoreSpaceBlockContext> for State {
    fn from(context: simulation_core_space::CoreSpaceBlockContext) -> Self {
        Self {
            epoch_number: context.epoch_number.into(),
            pivot_hash: b256_to_wire(context.pivot_hash),
        }
    }
}

impl CompletedTransaction {
    fn try_from_simulation(
        transaction: simulation_core_space::CoreSpaceCompleteTransaction,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        let common = transaction.common();
        let common = TransactionCommon {
            chain_id: u64::from(common.chain_id).into(),
            from: map_core_address(common.from, network, "transaction.from".to_owned())?,
            to: common
                .to
                .map(|address| map_core_address(address, network, "transaction.to".to_owned()))
                .transpose()?,
            nonce: u256_to_wire(common.nonce),
            gas: u256_to_wire(common.gas_limit),
            value: u256_to_wire(common.value),
            data: CoreSpaceRpcBytes::from(common.data.to_vec()),
            storage_limit: common.storage_limit.into(),
            epoch_height: common.epoch_height.into(),
        };

        match transaction {
            simulation_core_space::CoreSpaceCompleteTransaction::Cip155 { gas_price, .. } => {
                Ok(Self::Cip155 {
                    common,
                    gas_price: u256_to_wire(gas_price),
                })
            }
            simulation_core_space::CoreSpaceCompleteTransaction::Cip2930 {
                gas_price,
                access_list,
                ..
            } => Ok(Self::Cip2930 {
                common,
                gas_price: u256_to_wire(gas_price),
                access_list: map_access_list(access_list, network)?,
            }),
            simulation_core_space::CoreSpaceCompleteTransaction::Cip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
                ..
            } => Ok(Self::Cip1559 {
                common,
                max_fee_per_gas: u256_to_wire(max_fee_per_gas),
                max_priority_fee_per_gas: u256_to_wire(max_priority_fee_per_gas),
                access_list: map_access_list(access_list, network)?,
            }),
        }
    }
}

fn map_access_list(
    access_list: Vec<simulation_core_space::CoreSpaceAccessListItem>,
    network: Network,
) -> Result<Vec<AccessListItem>, ResponseMappingError> {
    access_list
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            Ok(AccessListItem {
                address: map_core_address(
                    item.address,
                    network,
                    format!("transaction.accessList[{index}].address"),
                )?,
                storage_keys: item.storage_keys.into_iter().map(b256_to_wire).collect(),
            })
        })
        .collect()
}

impl Outcome {
    fn try_from_simulation(
        outcome: simulation_core_space::CoreSpaceExecutionOutcome,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        match outcome {
            simulation_core_space::CoreSpaceExecutionOutcome::Success {
                result,
                output,
                logs,
            } => Ok(Self::Success {
                accounting: result.into(),
                output: SuccessOutput::try_from_simulation(output, network)?,
                logs: logs
                    .into_iter()
                    .enumerate()
                    .map(|(index, log)| SimulationLog::try_from_simulation(log, network, index))
                    .collect::<Result<_, _>>()?,
            }),
            simulation_core_space::CoreSpaceExecutionOutcome::Reverted {
                result,
                revert_data,
                reason,
            } => Ok(Self::Reverted {
                accounting: result.into(),
                revert_data: CoreSpaceRpcBytes::from(revert_data.to_vec()),
                reason: reason.map(|reason| reason.to_string()),
            }),
            simulation_core_space::CoreSpaceExecutionOutcome::Failed { result, failure } => {
                Ok(Self::Failed {
                    accounting: result.into(),
                    error: failure.to_string(),
                })
            }
            simulation_core_space::CoreSpaceExecutionOutcome::NotExecuted(rejection) => {
                Ok(Self::Rejected {
                    error: rejection.to_string(),
                })
            }
        }
    }
}

impl From<simulation_core_space::CoreSpaceExecutionResult> for ExecutionAccounting {
    fn from(result: simulation_core_space::CoreSpaceExecutionResult) -> Self {
        Self {
            gas_used: result.gas().gas_used().into(),
            gas_fee: u256_to_wire(result.gas_fee()),
            burnt_gas_fee: result.burnt_gas_fee().map(u256_to_wire),
            effective_gas_price: u256_to_wire(result.effective_gas_price()),
            gas_covered_by_sponsor: result.gas_covered_by_sponsor(),
            storage_collateralized: result.storage_collateralized().into(),
            storage_covered_by_sponsor: result.storage_covered_by_sponsor(),
        }
    }
}

impl SuccessOutput {
    fn try_from_simulation(
        output: simulation_core_space::CoreSpaceSuccessOutput,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        match output {
            simulation_core_space::CoreSpaceSuccessOutput::Call { return_data } => Ok(Self::Call {
                return_data: CoreSpaceRpcBytes::from(return_data.to_vec()),
            }),
            simulation_core_space::CoreSpaceSuccessOutput::Create {
                address,
                runtime_code,
            } => Ok(Self::Create {
                contract_address: map_core_address(
                    address,
                    network,
                    "outcome.contractAddress".to_owned(),
                )?,
                runtime_code: CoreSpaceRpcBytes::from(runtime_code.to_vec()),
            }),
        }
    }
}

impl SimulationLog {
    fn try_from_simulation(
        log: simulation_core_space::CoreSpaceLog,
        network: Network,
        index: usize,
    ) -> Result<Self, ResponseMappingError> {
        let address = match log.address {
            simulation_core_space::CoreSpaceLogAddress::CoreSpace(address) => {
                LogAddress::CoreSpace(map_core_address(
                    address,
                    network,
                    format!("outcome.logs[{index}].address"),
                )?)
            }
            simulation_core_space::CoreSpaceLogAddress::Espace(address) => {
                LogAddress::Espace(address)
            }
        };
        Ok(Self {
            address,
            topics: log.topics.into_iter().map(b256_to_wire).collect(),
            data: CoreSpaceRpcBytes::from(log.data.to_vec()),
        })
    }
}

impl Changes {
    fn try_from_simulation(
        changes: simulation_core_space::CoreSpaceChanges,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        match changes {
            simulation_core_space::CoreSpaceChanges::Complete(changes) => Ok(Self::Complete {
                items: core_space_change::try_map_changes(changes.into_items(), network)?,
            }),
            simulation_core_space::CoreSpaceChanges::Unavailable(error) => Ok(Self::Unavailable {
                error: error.to_string(),
            }),
        }
    }
}

pub(super) fn map_core_address(
    address: CoreAddress,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    if address.network() != provider_network(network) {
        return Err(ResponseMappingError {
            field,
            message: format!(
                "address uses network {}, expected {network}",
                address.network()
            ),
        });
    }

    map_core_space_address(Address::from_slice(&address.bytes()), network, field)
}

fn provider_network(network: Network) -> conflux_provider::Network {
    match network {
        Network::Main => conflux_provider::Network::Main,
        Network::Test => conflux_provider::Network::Test,
        Network::Id(id) => conflux_provider::Network::Id(id),
    }
}

pub(super) fn map_core_space_address(
    address: Address,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    RpcAddress::try_from_h160(address, network)
        .map_err(|message| ResponseMappingError { field, message })
}
