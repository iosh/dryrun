use std::sync::Arc;

use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError, PreparedCoreSpaceSimulation, PreparedEspaceSimulation,
    config::ConfluxChainConfig,
    core_space::{
        CoreSpaceExecution, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
        CoreSpaceStateAnchor, CoreSpaceTransaction, CoreSpaceTransactionVariant,
        SimulateCoreSpaceTransactionInput, build_core_space_not_executed,
        build_core_space_transaction_input,
    },
    espace::{
        EspaceExecution, SimulateEspaceTransactionInput, build_espace_not_executed,
        build_espace_transaction_input, validate_espace_transaction,
    },
    execution::{DryRunTransactionInput, TransactionExecutionInput},
    preparation::context::{
        resolve_core_space_execution_context, resolve_espace_execution_context,
    },
    preparation::{
        PreparedCoreSpaceSimulationState, PreparedEspaceSimulationState, ReadyCoreSpaceSimulation,
        ReadyEspaceSimulation,
    },
    state::{ConfluxStatePoint, RemoteStateProvider, RemoteStateReader},
};

pub struct ConfluxEngine {
    chain: ConfluxChainConfig,
    provider: Arc<dyn RemoteStateProvider>,
    runtime_handle: Handle,
}

impl ConfluxEngine {
    pub fn new(
        chain: ConfluxChainConfig,
        provider: Arc<dyn RemoteStateProvider>,
        runtime_handle: Handle,
    ) -> Self {
        Self {
            chain,
            provider,
            runtime_handle,
        }
    }

    pub async fn prepare_espace_transaction(
        &self,
        input: SimulateEspaceTransactionInput,
    ) -> Result<PreparedEspaceSimulation, ConfluxEngineError> {
        let SimulateEspaceTransactionInput { block, transaction } = input;
        let gas_limit = transaction.gas_limit;
        let execution_context =
            resolve_espace_execution_context(self.provider.as_ref(), &block).await?;
        let chain_id = self.chain.evm_chain_id;

        if let Err(failure) = validate_espace_transaction(&transaction, chain_id) {
            return Ok(PreparedEspaceSimulation {
                kind: PreparedEspaceSimulationState::Complete(Box::new(build_espace_not_executed(
                    chain_id,
                    execution_context.simulated_block,
                    gas_limit,
                    failure,
                ))),
            });
        }

        let transaction = build_espace_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction: DryRunTransactionInput::Espace(transaction),
        };
        let state_reader = self
            .prepare_state_reader(execution_context.state_point)
            .await?;

        Ok(PreparedEspaceSimulation {
            kind: PreparedEspaceSimulationState::Ready(Box::new(ReadyEspaceSimulation {
                chain_id,
                simulated_block: execution_context.simulated_block,
                gas_limit,
                execution_input,
                state_reader,
            })),
        })
    }

    pub fn simulate_espace_transaction(
        &self,
        prepared: PreparedEspaceSimulation,
    ) -> Result<EspaceExecution, ConfluxEngineError> {
        crate::espace::simulation::simulate(prepared, &self.runtime_handle)
    }

    pub async fn prepare_core_space_transaction(
        &self,
        input: SimulateCoreSpaceTransactionInput,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxEngineError> {
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
            return Ok(PreparedCoreSpaceSimulation {
                kind: PreparedCoreSpaceSimulationState::Complete(Box::new(
                    build_core_space_not_executed(chain_id, state_anchor, gas_limit, failure),
                )),
            });
        }

        let transaction = build_core_space_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction: DryRunTransactionInput::CoreSpace(transaction),
        };
        let state_reader = self
            .prepare_state_reader(execution_context.state_point)
            .await?;

        Ok(PreparedCoreSpaceSimulation {
            kind: PreparedCoreSpaceSimulationState::Ready(Box::new(ReadyCoreSpaceSimulation {
                chain_id,
                state_anchor,
                gas_limit,
                execution_input,
                state_reader,
            })),
        })
    }

    pub fn simulate_core_space_transaction(
        &self,
        prepared: PreparedCoreSpaceSimulation,
    ) -> Result<CoreSpaceExecution, ConfluxEngineError> {
        crate::core_space::simulation::simulate(prepared, &self.runtime_handle)
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
