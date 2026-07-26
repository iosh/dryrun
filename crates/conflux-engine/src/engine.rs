use std::sync::Arc;

use crate::{
    ConfluxEngineError,
    config::ConfluxChainConfig,
    core_space::{
        CoreSpaceExecution, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
        CoreSpaceStateAnchor, CoreSpaceTransaction, CoreSpaceTransactionVariant,
        SimulateCoreSpaceTransactionInput, build_core_space_execution,
        build_core_space_not_executed, build_core_space_transaction_input,
    },
    espace::{
        EspaceExecution, SimulateEspaceTransactionInput, build_espace_execution,
        build_espace_not_executed, build_espace_transaction_input, validate_espace_transaction,
    },
    execution::{
        DryRunTransactionInput, TransactionExecutionInput, build_mainnet_machine,
        build_rpc_backed_state, execute_transaction,
    },
    preparation::context::{
        resolve_core_space_execution_context, resolve_espace_execution_context,
    },
    state::{ConfluxStatePoint, RemoteStateProvider, RemoteStateReader},
};

pub struct ConfluxEngine {
    chain: ConfluxChainConfig,
    provider: Arc<dyn RemoteStateProvider>,
}

impl ConfluxEngine {
    pub fn new(chain: ConfluxChainConfig, provider: Arc<dyn RemoteStateProvider>) -> Self {
        Self { chain, provider }
    }

    pub async fn simulate_espace_transaction(
        &self,
        input: SimulateEspaceTransactionInput,
    ) -> Result<EspaceExecution, ConfluxEngineError> {
        let runtime_handle = current_runtime_handle()?;
        let SimulateEspaceTransactionInput { block, transaction } = input;
        let gas_limit = transaction.gas_limit;
        let execution_context =
            resolve_espace_execution_context(self.provider.as_ref(), &block).await?;
        let chain_id = self.chain.evm_chain_id;

        if let Err(failure) = validate_espace_transaction(&transaction, chain_id) {
            return Ok(build_espace_not_executed(
                chain_id,
                execution_context.simulated_block,
                gas_limit,
                failure,
            ));
        }

        let transaction = build_espace_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction: DryRunTransactionInput::Espace(transaction),
        };
        let state_reader = self
            .prepare_state_reader(execution_context.state_point)
            .await?;

        tokio::task::spawn_blocking(move || {
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            let machine = build_mainnet_machine();

            let outcome =
                execute_transaction(&mut state, &machine, execution_input).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            build_espace_execution(
                chain_id,
                execution_context.simulated_block,
                gas_limit,
                outcome,
            )
        })
        .await
        .map_err(|error| ConfluxEngineError::ExecutionInternal {
            message: format!("eSpace blocking execution task failed: {error}"),
        })?
    }

    pub async fn simulate_core_space_transaction(
        &self,
        input: SimulateCoreSpaceTransactionInput,
    ) -> Result<CoreSpaceExecution, ConfluxEngineError> {
        let runtime_handle = current_runtime_handle()?;
        let SimulateCoreSpaceTransactionInput { epoch, transaction } = input;
        let gas_limit = transaction.gas_limit;
        let execution_context =
            resolve_core_space_execution_context(self.provider.as_ref(), &epoch).await?;
        let chain_id = self.chain.core_space_chain_id;
        let state_anchor = CoreSpaceStateAnchor {
            epoch_number: execution_context.state_point.anchor().epoch_number(),
            pivot_hash: execution_context.state_point.anchor().pivot_hash(),
        };

        if let Err(failure) = validate_core_space_transaction(&transaction, chain_id) {
            return Ok(build_core_space_not_executed(
                chain_id,
                state_anchor,
                gas_limit,
                failure,
            ));
        }

        let transaction = build_core_space_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction: DryRunTransactionInput::CoreSpace(transaction),
        };
        let state_reader = self
            .prepare_state_reader(execution_context.state_point)
            .await?;

        tokio::task::spawn_blocking(move || {
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            let machine = build_mainnet_machine();

            let outcome =
                execute_transaction(&mut state, &machine, execution_input).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            Ok(build_core_space_execution(
                chain_id,
                state_anchor,
                gas_limit,
                outcome,
            ))
        })
        .await
        .map_err(|error| ConfluxEngineError::ExecutionInternal {
            message: format!("Core Space blocking execution task failed: {error}"),
        })?
    }

    async fn prepare_state_reader(
        &self,
        state_point: ConfluxStatePoint,
    ) -> Result<RemoteStateReader, ConfluxEngineError> {
        RemoteStateReader::prepare(state_point, Arc::clone(&self.provider))
            .await
            .map_err(|error| ConfluxEngineError::StateAccess {
                message: error.to_string(),
            })
    }
}

fn current_runtime_handle() -> Result<tokio::runtime::Handle, ConfluxEngineError> {
    tokio::runtime::Handle::try_current().map_err(|error| ConfluxEngineError::ExecutionInternal {
        message: format!("Conflux simulation requires a Tokio runtime: {error}"),
    })
}

fn validate_core_space_transaction(
    transaction: &CoreSpaceTransaction,
    expected_chain_id: u32,
) -> Result<(), CoreSpaceExecutionFailure> {
    if transaction.chain_id != expected_chain_id {
        return Err(CoreSpaceExecutionFailure {
            code: CoreSpaceExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match engine chain id {}",
                transaction.chain_id, expected_chain_id
            ),
            reason: None,
        });
    }

    match &transaction.variant {
        CoreSpaceTransactionVariant::Cip155 { gas_price }
        | CoreSpaceTransactionVariant::Cip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Err(CoreSpaceExecutionFailure {
                    code: CoreSpaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        CoreSpaceTransactionVariant::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if max_fee_per_gas.is_zero() {
                return Err(CoreSpaceExecutionFailure {
                    code: CoreSpaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction max fee per gas must be greater than zero".to_string(),
                    reason: None,
                });
            }

            if max_priority_fee_per_gas > max_fee_per_gas {
                return Err(CoreSpaceExecutionFailure {
                    code: CoreSpaceExecutionFailureCode::PriorityFeeExceedsMaxFee,
                    message: format!(
                        "max priority fee per gas {} exceeds max fee per gas {}",
                        max_priority_fee_per_gas, max_fee_per_gas
                    ),
                    reason: None,
                });
            }
        }
    }

    Ok(())
}
