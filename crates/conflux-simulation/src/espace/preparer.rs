use crate::{
    ConfluxSimulationBackend, ConfluxSimulationError, PreparedEspaceSimulation,
    execution::{DryRunTransactionInput, TransactionExecutionInput},
    preparation::{
        PreparedEspaceSimulationState, ReadyEspaceSimulation, complete_espace_transaction,
    },
};

use super::{
    EspaceBlockSelector, EspaceTransaction, ResolvedEspaceContext, build_espace_not_executed,
    build_espace_transaction_input, resolve_espace_context, validate_espace_transaction,
};

#[derive(Clone)]
pub struct EspaceSimulationPreparer {
    backend: ConfluxSimulationBackend,
}

impl EspaceSimulationPreparer {
    pub fn new(backend: ConfluxSimulationBackend) -> Self {
        Self { backend }
    }

    pub async fn prepare_transaction(
        &self,
        block: EspaceBlockSelector,
        request: simulation_transaction::TransactionRequest,
    ) -> Result<PreparedEspaceSimulation, ConfluxSimulationError> {
        let context = resolve_espace_context(self.backend.provider(), block).await?;
        let transaction =
            complete_espace_transaction(self.backend.provider(), &context, request).await?;
        self.prepare_completed_transaction(context, transaction)
            .await
    }

    async fn prepare_completed_transaction(
        &self,
        context: ResolvedEspaceContext,
        transaction: EspaceTransaction,
    ) -> Result<PreparedEspaceSimulation, ConfluxSimulationError> {
        let gas_limit = transaction.gas_limit;
        let chain_id = self.backend.chain_spec().espace_chain_id();

        if let Err(failure) = validate_espace_transaction(&transaction, chain_id) {
            return Ok(PreparedEspaceSimulation {
                state: PreparedEspaceSimulationState::Finished(Box::new(
                    build_espace_not_executed(chain_id, context.public_context, gas_limit, failure),
                )),
            });
        }

        let transaction = build_espace_transaction_input(transaction, chain_id);
        let execution_input = TransactionExecutionInput {
            block_context: context.execution_block_context,
            transaction: DryRunTransactionInput::Espace(transaction),
        };
        let state_source = self
            .backend
            .prepare_state_source(context.state_anchor)
            .await?;

        Ok(PreparedEspaceSimulation {
            state: PreparedEspaceSimulationState::Ready(Box::new(ReadyEspaceSimulation {
                backend: self.backend.clone(),
                chain_id,
                simulated_block: context.public_context,
                gas_limit,
                execution_input,
                state_source,
            })),
        })
    }
}
