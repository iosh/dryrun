use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_provider::CoreAddress;
use conflux_simulation::core_space as simulation_core_space;
use serde::Serialize;

use super::{core_space_change, u256_to_wire};

#[derive(Debug, thiserror::Error)]
#[error("failed to encode `{field}` as a Core Space address: {message}")]
pub(crate) struct ResponseMappingError {
    field: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateCoreSpaceTransactionResponse {
    execution: CoreSpaceExecution,
    changes: Vec<core_space_change::Change>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceExecution {
    chain_id: U64,
    state: CoreSpaceStateAnchor,
    status: CoreSpaceExecutionStatus,
    gas_used: U256,
    gas_limit: U256,
    gas_charged: U256,
    fee: U256,
    burnt_fee: Option<U256>,
    gas_covered_by_sponsor: bool,
    storage_covered_by_sponsor: bool,
    output: CoreSpaceRpcBytes,
    failure: Option<CoreSpaceExecutionFailure>,
}

struct ExecutedWireFields {
    gas_used: U256,
    gas_charged: U256,
    fee: U256,
    burnt_fee: Option<U256>,
    gas_covered_by_sponsor: bool,
    storage_covered_by_sponsor: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceStateAnchor {
    epoch_number: U64,
    pivot_hash: H256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CoreSpaceExecutionStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceExecutionFailure {
    code: CoreSpaceExecutionFailureCode,
    message: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CoreSpaceExecutionFailureCode {
    ChainIdMismatch,
    ZeroGasPrice,
    PriorityFeeExceedsMaxFee,
    TransactionTypeNotActivated,
    NonceTooLow,
    NonceTooHigh,
    EpochHeightOutOfBound,
    FeeBelowBaseFee,
    IntrinsicGasTooLow,
    InvalidRecipient,
    SenderWithCode,
    SenderDoesNotExist,
    InsufficientFunds,
    SponsorBalanceInsufficient,
    Revert,
    OutOfGas,
    StorageBalanceInsufficient,
    StorageLimitExceeded,
    NonceOverflow,
    VmError,
}

impl SimulateCoreSpaceTransactionResponse {
    pub(crate) fn try_from_output(
        simulation: simulation_core_space::CoreSpaceSimulation,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        let (_, _, execution, changes) = simulation.into_parts();
        Ok(Self {
            execution: CoreSpaceExecution::from_simulation(execution),
            changes: core_space_change::try_map_changes(changes, network)?,
        })
    }
}

impl CoreSpaceExecution {
    fn from_simulation(execution: simulation_core_space::CoreSpaceExecution) -> Self {
        let simulation_core_space::CoreSpaceExecution {
            chain_id,
            context: state,
            gas_limit,
            outcome,
        } = execution;
        let (
            status,
            gas_used,
            gas_charged,
            fee,
            burnt_fee,
            gas_covered_by_sponsor,
            storage_covered_by_sponsor,
            output,
            failure,
        ) = match outcome {
            simulation_core_space::CoreSpaceExecutionOutcome::Success {
                result, output, ..
            } => {
                let fields = ExecutedWireFields::from_simulation(&result);
                let output = match output {
                    simulation_core_space::CoreSpaceSuccessOutput::Call { return_data } => {
                        return_data
                    }
                    simulation_core_space::CoreSpaceSuccessOutput::Create {
                        runtime_code, ..
                    } => runtime_code,
                };
                (
                    CoreSpaceExecutionStatus::Success,
                    fields.gas_used,
                    fields.gas_charged,
                    fields.fee,
                    fields.burnt_fee,
                    fields.gas_covered_by_sponsor,
                    fields.storage_covered_by_sponsor,
                    CoreSpaceRpcBytes::from(output.to_vec()),
                    None,
                )
            }
            simulation_core_space::CoreSpaceExecutionOutcome::Reverted {
                result,
                revert_data,
                reason: _,
            } => {
                let fields = ExecutedWireFields::from_simulation(&result);
                (
                    CoreSpaceExecutionStatus::Failed,
                    fields.gas_used,
                    fields.gas_charged,
                    fields.fee,
                    fields.burnt_fee,
                    fields.gas_covered_by_sponsor,
                    fields.storage_covered_by_sponsor,
                    CoreSpaceRpcBytes::from(revert_data.to_vec()),
                    Some(CoreSpaceExecutionFailure {
                        code: CoreSpaceExecutionFailureCode::Revert,
                        message: "execution reverted".to_string(),
                        reason: None,
                    }),
                )
            }
            simulation_core_space::CoreSpaceExecutionOutcome::Failed { result, failure } => {
                let fields = ExecutedWireFields::from_simulation(&result);
                (
                    CoreSpaceExecutionStatus::Failed,
                    fields.gas_used,
                    fields.gas_charged,
                    fields.fee,
                    fields.burnt_fee,
                    fields.gas_covered_by_sponsor,
                    fields.storage_covered_by_sponsor,
                    CoreSpaceRpcBytes::default(),
                    Some(CoreSpaceExecutionFailure {
                        code: wire_failure_code_for_execution_failure(&failure),
                        message: wire_message_for_execution_failure(&failure),
                        reason: None,
                    }),
                )
            }
            simulation_core_space::CoreSpaceExecutionOutcome::NotExecuted(rejection) => (
                CoreSpaceExecutionStatus::NotExecuted,
                U256::zero(),
                U256::zero(),
                U256::zero(),
                Some(U256::zero()),
                false,
                false,
                CoreSpaceRpcBytes::default(),
                Some(CoreSpaceExecutionFailure {
                    code: wire_failure_code_for_rejection(&rejection),
                    message: wire_message_for_rejection(&rejection),
                    reason: None,
                }),
            ),
        };

        Self {
            chain_id: chain_id.into(),
            state: state.into(),
            status,
            gas_used,
            gas_limit: u256_to_wire(gas_limit),
            gas_charged,
            fee,
            burnt_fee,
            gas_covered_by_sponsor,
            storage_covered_by_sponsor,
            output,
            failure,
        }
    }
}

impl ExecutedWireFields {
    fn from_simulation(result: &simulation_core_space::CoreSpaceExecutionResult) -> Self {
        Self {
            gas_used: result.gas().gas_used().into(),
            gas_charged: result.gas().gas_charged().into(),
            fee: u256_to_wire(result.gas_fee()),
            burnt_fee: result.burnt_gas_fee().map(u256_to_wire),
            gas_covered_by_sponsor: result.gas_covered_by_sponsor(),
            storage_covered_by_sponsor: result.storage_covered_by_sponsor(),
        }
    }
}

impl From<simulation_core_space::CoreSpaceBlockContext> for CoreSpaceStateAnchor {
    fn from(state: simulation_core_space::CoreSpaceBlockContext) -> Self {
        Self {
            epoch_number: state.epoch_number.into(),
            pivot_hash: H256::from_slice(state.pivot_hash.as_slice()),
        }
    }
}

fn wire_failure_code_for_rejection(
    rejection: &simulation_core_space::CoreSpaceTransactionRejection,
) -> CoreSpaceExecutionFailureCode {
    use simulation_core_space::CoreSpaceTransactionRejection as Rejection;

    match rejection {
        Rejection::InvalidChainId { .. } => CoreSpaceExecutionFailureCode::ChainIdMismatch,
        Rejection::ZeroGasPrice | Rejection::ZeroMaxFeePerGas => {
            CoreSpaceExecutionFailureCode::ZeroGasPrice
        }
        Rejection::PriorityFeeGreaterThanMaxFee { .. } => {
            CoreSpaceExecutionFailureCode::PriorityFeeExceedsMaxFee
        }
        Rejection::Cip2930NotActivated | Rejection::Cip1559NotActivated => {
            CoreSpaceExecutionFailureCode::TransactionTypeNotActivated
        }
        Rejection::NonceTooLow { .. } => CoreSpaceExecutionFailureCode::NonceTooLow,
        Rejection::NonceTooHigh { .. } => CoreSpaceExecutionFailureCode::NonceTooHigh,
        Rejection::EpochHeightOutOfBounds { .. } => {
            CoreSpaceExecutionFailureCode::EpochHeightOutOfBound
        }
        Rejection::IntrinsicGasExceedsGasLimit { .. } => {
            CoreSpaceExecutionFailureCode::IntrinsicGasTooLow
        }
        Rejection::InvalidRecipient { .. } => CoreSpaceExecutionFailureCode::InvalidRecipient,
        Rejection::SenderHasCode { .. } => CoreSpaceExecutionFailureCode::SenderWithCode,
        Rejection::SenderDoesNotExist => CoreSpaceExecutionFailureCode::SenderDoesNotExist,
        Rejection::GasPriceBelowBaseFee { .. } => CoreSpaceExecutionFailureCode::FeeBelowBaseFee,
        Rejection::InsufficientFunds { .. } => CoreSpaceExecutionFailureCode::InsufficientFunds,
        Rejection::SponsorBalanceInsufficient { .. } => {
            CoreSpaceExecutionFailureCode::SponsorBalanceInsufficient
        }
        _ => CoreSpaceExecutionFailureCode::VmError,
    }
}

fn wire_failure_code_for_execution_failure(
    failure: &simulation_core_space::CoreSpaceExecutionFailure,
) -> CoreSpaceExecutionFailureCode {
    use simulation_core_space::CoreSpaceExecutionFailure as Failure;

    match failure {
        Failure::InsufficientFunds { .. } => CoreSpaceExecutionFailureCode::InsufficientFunds,
        Failure::OutOfGas => CoreSpaceExecutionFailureCode::OutOfGas,
        Failure::StorageBalanceInsufficient { .. } => {
            CoreSpaceExecutionFailureCode::StorageBalanceInsufficient
        }
        Failure::StorageLimitExceeded => CoreSpaceExecutionFailureCode::StorageLimitExceeded,
        Failure::NonceOverflow { .. } => CoreSpaceExecutionFailureCode::NonceOverflow,
        Failure::InvalidJump { .. }
        | Failure::InvalidInstruction { .. }
        | Failure::StackUnderflow { .. }
        | Failure::StackOverflow { .. }
        | Failure::SubroutineStackUnderflow { .. }
        | Failure::SubroutineStackOverflow { .. }
        | Failure::InvalidSubroutineEntry
        | Failure::BuiltInContract { .. }
        | Failure::InternalContract { .. }
        | Failure::StateChangeDuringStaticCall
        | Failure::CreateInitCodeSizeLimit
        | Failure::Wasm { .. }
        | Failure::ReturnDataOutOfBounds
        | Failure::InvalidAddress { .. }
        | Failure::CreateCollision { .. }
        | Failure::CreateContractStartingWithEf => CoreSpaceExecutionFailureCode::VmError,
        _ => CoreSpaceExecutionFailureCode::VmError,
    }
}

fn wire_message_for_rejection(
    rejection: &simulation_core_space::CoreSpaceTransactionRejection,
) -> String {
    use simulation_core_space::CoreSpaceTransactionRejection as Rejection;

    match rejection {
        Rejection::InvalidChainId {
            transaction_chain_id,
            expected_chain_id,
        } => format!(
            "transaction chain id {transaction_chain_id} does not match simulation chain id {expected_chain_id}"
        ),
        Rejection::ZeroGasPrice => "transaction gas price must be greater than zero".to_string(),
        Rejection::ZeroMaxFeePerGas => {
            "transaction max fee per gas must be greater than zero".to_string()
        }
        Rejection::Cip2930NotActivated | Rejection::Cip1559NotActivated => {
            "typed Core Space transactions are not active in the simulation context".to_string()
        }
        Rejection::InvalidRecipient { recipient } => format!(
            "invalid Core Space recipient address: {:?}",
            raw_core_address(*recipient)
        ),
        Rejection::SenderHasCode { sender } => format!(
            "transaction sender has contract code: {:?}",
            raw_core_address(*sender)
        ),
        Rejection::GasPriceBelowBaseFee {
            gas_price,
            base_fee_per_gas,
        } => format!(
            "transaction gas price {gas_price} is lower than required base fee {base_fee_per_gas}"
        ),
        _ => rejection.to_string(),
    }
}

fn wire_message_for_execution_failure(
    failure: &simulation_core_space::CoreSpaceExecutionFailure,
) -> String {
    use simulation_core_space::CoreSpaceExecutionFailure as Failure;

    match failure {
        Failure::InsufficientFunds {
            required,
            available,
            actual_gas_cost,
            maximum_storage_cost,
        } => format!(
            "sender balance {available} is lower than required cost {required}; actual gas cost is {actual_gas_cost}, maximum storage cost is {maximum_storage_cost}"
        ),
        Failure::OutOfGas => "execution ran out of gas".to_string(),
        Failure::StorageBalanceInsufficient {
            required,
            available,
        } => format!(
            "storage collateral balance {available} is lower than required amount {required}"
        ),
        Failure::StorageLimitExceeded => {
            "execution exceeded the transaction storage limit".to_string()
        }
        Failure::NonceOverflow { address } => {
            format!(
                "nonce overflow for address: {:?}",
                raw_core_address(*address)
            )
        }
        Failure::InvalidJump { destination } => {
            format!("virtual machine execution failed: Bad jump destination {destination:x}")
        }
        Failure::InvalidInstruction { instruction } => {
            format!("virtual machine execution failed: Bad instruction {instruction:x}")
        }
        Failure::StackUnderflow {
            instruction,
            wanted,
            available,
        } => format!(
            "virtual machine execution failed: Stack underflow {instruction} {wanted}/{available}"
        ),
        Failure::StackOverflow {
            instruction,
            wanted,
            limit,
        } => {
            format!("virtual machine execution failed: Out of stack {instruction} {wanted}/{limit}")
        }
        Failure::SubroutineStackUnderflow { wanted, available } => format!(
            "virtual machine execution failed: Subroutine stack underflow {wanted}/{available}"
        ),
        Failure::SubroutineStackOverflow { wanted, limit } => {
            format!("virtual machine execution failed: Out of subroutine stack {wanted}/{limit}")
        }
        Failure::InvalidSubroutineEntry => {
            "virtual machine execution failed: Invalid Subroutine Entry via BEGINSUB".to_string()
        }
        Failure::BuiltInContract { details } => {
            format!("virtual machine execution failed: Built-in failed: {details}")
        }
        Failure::InternalContract { details } => {
            format!("virtual machine execution failed: InternalContract failed: {details}")
        }
        Failure::StateChangeDuringStaticCall => {
            "virtual machine execution failed: Mutable call in static context".to_string()
        }
        Failure::CreateInitCodeSizeLimit => {
            "virtual machine execution failed: Exceed create initcode size limit".to_string()
        }
        Failure::Wasm { details } => {
            format!("virtual machine execution failed: Internal error: {details}")
        }
        Failure::ReturnDataOutOfBounds => {
            "virtual machine execution failed: Out of bounds".to_string()
        }
        Failure::InvalidAddress { address } => format!(
            "virtual machine execution failed: InvalidAddress: {}",
            raw_core_address(*address)
        ),
        Failure::CreateCollision { address } => format!(
            "virtual machine execution failed: Contract creation on an existing address: {}",
            raw_core_address(*address)
        ),
        Failure::CreateContractStartingWithEf => {
            "virtual machine execution failed: Create contract starting with EF".to_string()
        }
        _ => failure.to_string(),
    }
}

fn raw_core_address(address: simulation_core_space::CoreAddress) -> Address {
    Address::from_slice(&address.bytes())
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
