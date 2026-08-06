use std::sync::Arc;

use crate::{
    ConfluxSimulationError, ConfluxSimulationProvider, PreparedEspaceSimulation,
    config::ConfluxChainConfig,
    execution::{DryRunTransactionInput, TransactionExecutionInput},
    preparation::{
        EspaceSimulationContext, PreparedEspaceSimulationState, ReadyEspaceSimulation,
        complete_espace_transaction, load_espace_context, prepare_state_source,
    },
};

use super::{
    EspaceBlockRef, EspaceTransaction, build_espace_not_executed, build_espace_transaction_input,
    validate_espace_transaction,
};

#[derive(Clone)]
pub struct EspaceSimulationPreparer {
    chain: ConfluxChainConfig,
    provider: Arc<ConfluxSimulationProvider>,
}

impl EspaceSimulationPreparer {
    pub fn new(chain: ConfluxChainConfig, provider: Arc<ConfluxSimulationProvider>) -> Self {
        Self { chain, provider }
    }

    pub async fn prepare_transaction(
        &self,
        block: EspaceBlockRef,
        request: simulation_transaction::TransactionRequest,
    ) -> Result<PreparedEspaceSimulation, ConfluxSimulationError> {
        let context = load_espace_context(self.provider.as_ref(), &block).await?;
        let transaction =
            complete_espace_transaction(self.provider.as_ref(), &context, request).await?;
        self.prepare_completed_transaction(context, transaction)
            .await
    }

    async fn prepare_completed_transaction(
        &self,
        context: EspaceSimulationContext,
        transaction: EspaceTransaction,
    ) -> Result<PreparedEspaceSimulation, ConfluxSimulationError> {
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
        let state_source =
            prepare_state_source(Arc::clone(&self.provider), context.state_anchor).await?;

        Ok(PreparedEspaceSimulation {
            state: PreparedEspaceSimulationState::Ready(Box::new(ReadyEspaceSimulation {
                chain_id,
                simulated_block: context.simulated_block,
                gas_limit,
                execution_input,
                state_source,
            })),
        })
    }
}
