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

impl ResponseMappingError {
    pub(super) fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateCoreSpaceTransactionResponse {
    state: State,
    transaction: simulation_core_space::CoreSpaceTypedTransaction,
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
        items: Vec<core_space_change::WireChangeItem>,
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
            transaction,
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
