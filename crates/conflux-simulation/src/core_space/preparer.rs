use crate::{
    ConfluxSimulationBackend, ConfluxSimulationError, PreparedCoreSpaceSimulation,
    chain_spec::CoreSpaceTransactionValidationRules,
    execution::{next_execution_block_number, next_execution_epoch_height},
    preparation::{
        FinishedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
};

use super::{
    CoreSpaceBlockSelector, CoreSpaceCompleteTransaction, CoreSpaceCompleteTransactionVariant,
    CoreSpaceTransactionInput, CoreSpaceTransactionRejection, ResolvedCoreSpaceContext,
    build_core_space_not_executed, complete_transaction, resolve_core_space_context,
    resolve_storage_sponsorship,
};

#[derive(Clone)]
pub struct CoreSpaceSimulationPreparer {
    backend: ConfluxSimulationBackend,
}

impl CoreSpaceSimulationPreparer {
    pub fn new(backend: ConfluxSimulationBackend) -> Self {
        Self { backend }
    }

    pub async fn prepare_transaction(
        &self,
        selector: CoreSpaceBlockSelector,
        transaction: CoreSpaceTransactionInput,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxSimulationError> {
        transaction.validate_network(self.backend.core_space_address_network())?;
        let context = resolve_core_space_context(self.backend.provider(), selector).await?;
        let chain_id = self.backend.chain_spec().core_space_chain_id();
        let transaction =
            complete_transaction(transaction, self.backend.provider(), &context, chain_id).await?;
        self.prepare_completed_transaction(context, transaction)
            .await
    }

    async fn prepare_completed_transaction(
        &self,
        context: ResolvedCoreSpaceContext,
        transaction: CoreSpaceCompleteTransaction,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxSimulationError> {
        let gas_limit = transaction.gas_limit;
        let chain_id = self.backend.chain_spec().core_space_chain_id();
        let execution_block_number =
            next_execution_block_number(context.execution_block_context.pivot_block_number)?;
        let execution_epoch_height =
            next_execution_epoch_height(context.execution_block_context.pivot_epoch_height)?;
        let rules = self
            .backend
            .chain_spec()
            .core_space_transaction_validation_rules(
                execution_block_number,
                execution_epoch_height,
            );
        let public_context = context.public_context;

        if let Some(rejection) = pre_execution_rejection(&transaction, chain_id, rules) {
            return Ok(PreparedCoreSpaceSimulation {
                state: PreparedCoreSpaceSimulationState::Finished(Box::new(
                    FinishedCoreSpaceSimulation {
                        context: public_context,
                        transaction,
                        execution: build_core_space_not_executed(
                            chain_id,
                            public_context,
                            gas_limit,
                            rejection,
                        ),
                    },
                )),
            });
        }

        let storage_sponsorship = resolve_storage_sponsorship(
            self.backend.provider(),
            context.state_anchor,
            &transaction,
        )
        .await?;
        let state_source = self
            .backend
            .prepare_state_source(context.state_anchor)
            .await?;

        Ok(PreparedCoreSpaceSimulation {
            state: PreparedCoreSpaceSimulationState::Ready(Box::new(ReadyCoreSpaceSimulation {
                backend: self.backend.clone(),
                chain_id,
                public_context,
                transaction,
                storage_sponsorship,
                execution_block_context: context.execution_block_context,
                state_source,
            })),
        })
    }
}

fn pre_execution_rejection(
    transaction: &CoreSpaceCompleteTransaction,
    expected_chain_id: u32,
    rules: CoreSpaceTransactionValidationRules,
) -> Option<CoreSpaceTransactionRejection> {
    if transaction.chain_id != expected_chain_id {
        return Some(CoreSpaceTransactionRejection::InvalidChainId {
            transaction_chain_id: transaction.chain_id,
            expected_chain_id,
        });
    }

    if !rules.typed_transactions_active {
        match &transaction.variant {
            CoreSpaceCompleteTransactionVariant::Cip155 { .. } => {}
            CoreSpaceCompleteTransactionVariant::Cip2930 { .. } => {
                return Some(CoreSpaceTransactionRejection::Cip2930NotActivated);
            }
            CoreSpaceCompleteTransactionVariant::Cip1559 { .. } => {
                return Some(CoreSpaceTransactionRejection::Cip1559NotActivated);
            }
        }
    }

    match &transaction.variant {
        CoreSpaceCompleteTransactionVariant::Cip155 { gas_price }
        | CoreSpaceCompleteTransactionVariant::Cip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Some(CoreSpaceTransactionRejection::ZeroGasPrice);
            }
        }
        CoreSpaceCompleteTransactionVariant::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if max_fee_per_gas.is_zero() {
                return Some(CoreSpaceTransactionRejection::ZeroMaxFeePerGas);
            }

            if rules.priority_fee_cap_active && max_priority_fee_per_gas > max_fee_per_gas {
                return Some(
                    CoreSpaceTransactionRejection::PriorityFeeGreaterThanMaxFee {
                        max_priority_fee_per_gas: *max_priority_fee_per_gas,
                        max_fee_per_gas: *max_fee_per_gas,
                    },
                );
            }
        }
    }

    None
}
