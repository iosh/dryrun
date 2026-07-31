use std::sync::Arc;

use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError, PreparedCoreSpaceSimulation, PreparedEspaceSimulation,
    config::ConfluxChainConfig,
    core_space::{
        CoreSpaceEpochRef, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
        CoreSpaceSimulation, CoreSpaceStateAnchor, CoreSpaceTransaction,
        CoreSpaceTransactionVariant, build_core_space_not_executed,
        build_core_space_transaction_input,
    },
    espace::{
        EspaceBlockRef, EspaceSimulation, EspaceTransaction, build_espace_not_executed,
        build_espace_transaction_input, validate_espace_transaction,
    },
    execution::{DryRunTransactionInput, TransactionExecutionInput},
    preparation::{
        CoreSpaceSimulationContext, EspaceSimulationContext, PreparedCoreSpaceSimulationState,
        PreparedEspaceSimulationState, ReadyCoreSpaceSimulation, ReadyEspaceSimulation,
        load_core_space_context, load_espace_context,
    },
    state::{ConfluxStateAnchor, HttpConfluxProvider, RemoteStateReader},
};

pub struct ConfluxEngine {
    chain: ConfluxChainConfig,
    provider: Arc<HttpConfluxProvider>,
    runtime_handle: Handle,
}

impl ConfluxEngine {
    pub fn new(
        chain: ConfluxChainConfig,
        provider: Arc<HttpConfluxProvider>,
        runtime_handle: Handle,
    ) -> Self {
        Self {
            chain,
            provider,
            runtime_handle,
        }
    }

    pub async fn load_espace_context(
        &self,
        block: EspaceBlockRef,
    ) -> Result<EspaceSimulationContext, ConfluxEngineError> {
        load_espace_context(self.provider.as_ref(), &block).await
    }

    pub async fn prepare_espace_transaction(
        &self,
        context: EspaceSimulationContext,
        transaction: EspaceTransaction,
    ) -> Result<PreparedEspaceSimulation, ConfluxEngineError> {
        let gas_limit = transaction.gas_limit;
        let chain_id = self.chain.evm_chain_id;

        if let Err(failure) = validate_espace_transaction(&transaction, chain_id) {
            return Ok(PreparedEspaceSimulation {
                state: PreparedEspaceSimulationState::Finished(Box::new(
                    build_espace_not_executed(
                        chain_id,
                        context.simulated_block,
                        gas_limit,
                        failure,
                    ),
                )),
            });
        }

        let transaction = build_espace_transaction_input(transaction, chain_id);

        let execution_input = TransactionExecutionInput {
            block_context: context.block_context,
            transaction: DryRunTransactionInput::Espace(transaction),
        };
        let state_reader = self.prepare_state_reader(context.state_anchor).await?;

        Ok(PreparedEspaceSimulation {
            state: PreparedEspaceSimulationState::Ready(Box::new(ReadyEspaceSimulation {
                chain_id,
                simulated_block: context.simulated_block,
                gas_limit,
                execution_input,
                state_reader,
            })),
        })
    }

    pub fn simulate_espace_transaction(
        &self,
        prepared: PreparedEspaceSimulation,
    ) -> Result<EspaceSimulation, ConfluxEngineError> {
        crate::espace::simulation::simulate(prepared, &self.runtime_handle)
    }

    pub async fn load_core_space_context(
        &self,
        epoch: CoreSpaceEpochRef,
    ) -> Result<CoreSpaceSimulationContext, ConfluxEngineError> {
        load_core_space_context(self.provider.as_ref(), &epoch).await
    }

    pub async fn prepare_core_space_transaction(
        &self,
        context: CoreSpaceSimulationContext,
        transaction: CoreSpaceTransaction,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxEngineError> {
        let gas_limit = transaction.transaction.gas_limit;
        let chain_id = self.chain.core_space_chain_id;
        let state_anchor = CoreSpaceStateAnchor {
            epoch_number: context.state_anchor.epoch_number(),
            pivot_hash: context.state_anchor.pivot_hash(),
        };

        if let Err(failure) = validate_core_space_transaction(&transaction, chain_id) {
            return Ok(PreparedCoreSpaceSimulation {
                state: PreparedCoreSpaceSimulationState::Finished(Box::new(
                    build_core_space_not_executed(chain_id, state_anchor, gas_limit, failure),
                )),
            });
        }

        let transaction = build_core_space_transaction_input(transaction, chain_id);

        let execution_input = TransactionExecutionInput {
            block_context: context.block_context,
            transaction: DryRunTransactionInput::CoreSpace(transaction),
        };
        let state_reader = self.prepare_state_reader(context.state_anchor).await?;

        Ok(PreparedCoreSpaceSimulation {
            state: PreparedCoreSpaceSimulationState::Ready(Box::new(ReadyCoreSpaceSimulation {
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
        prepared_simulation: PreparedCoreSpaceSimulation,
    ) -> Result<CoreSpaceSimulation, ConfluxEngineError> {
        crate::core_space::simulation::simulate(prepared_simulation, &self.runtime_handle)
    }

    async fn prepare_state_reader(
        &self,
        state_anchor: ConfluxStateAnchor,
    ) -> Result<RemoteStateReader, ConfluxEngineError> {
        RemoteStateReader::prepare(state_anchor, Arc::clone(&self.provider))
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
    let transaction = &transaction.transaction;

    if transaction.chain_id != u64::from(expected_chain_id) {
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
        CoreSpaceTransactionVariant::Legacy { gas_price }
        | CoreSpaceTransactionVariant::AccessList { gas_price, .. } => {
            if *gas_price == 0 {
                return Err(CoreSpaceExecutionFailure {
                    code: CoreSpaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        CoreSpaceTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if *max_fee_per_gas == 0 {
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
