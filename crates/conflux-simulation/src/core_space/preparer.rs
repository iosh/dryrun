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
    CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode, CoreSpaceTransactionInput,
    ResolvedCoreSpaceContext, build_core_space_not_executed, complete_transaction,
    prepare_storage_payer, resolve_core_space_context,
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

        if let Err(failure) = validate_core_space_transaction(&transaction, chain_id, rules) {
            return Ok(PreparedCoreSpaceSimulation {
                state: PreparedCoreSpaceSimulationState::Finished(Box::new(
                    FinishedCoreSpaceSimulation {
                        context: public_context,
                        transaction,
                        execution: build_core_space_not_executed(
                            chain_id,
                            public_context,
                            gas_limit,
                            failure,
                        ),
                    },
                )),
            });
        }

        let storage_payer =
            prepare_storage_payer(self.backend.provider(), context.state_anchor, &transaction)
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
                storage_payer,
                execution_block_context: context.execution_block_context,
                state_source,
            })),
        })
    }
}

fn validate_core_space_transaction(
    transaction: &CoreSpaceCompleteTransaction,
    expected_chain_id: u32,
    rules: CoreSpaceTransactionValidationRules,
) -> Result<(), CoreSpaceExecutionFailure> {
    if transaction.chain_id != expected_chain_id {
        return Err(CoreSpaceExecutionFailure {
            code: CoreSpaceExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match simulation chain id {}",
                transaction.chain_id, expected_chain_id
            ),
            reason: None,
        });
    }

    if !rules.typed_transactions_active
        && !matches!(
            transaction.variant,
            CoreSpaceCompleteTransactionVariant::Cip155 { .. }
        )
    {
        return Err(CoreSpaceExecutionFailure {
            code: CoreSpaceExecutionFailureCode::TransactionTypeNotActivated,
            message: "typed Core Space transactions are not active in the simulation context"
                .to_string(),
            reason: None,
        });
    }

    match &transaction.variant {
        CoreSpaceCompleteTransactionVariant::Cip155 { gas_price }
        | CoreSpaceCompleteTransactionVariant::Cip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Err(CoreSpaceExecutionFailure {
                    code: CoreSpaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        CoreSpaceCompleteTransactionVariant::Cip1559 {
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

            if rules.priority_fee_cap_active && max_priority_fee_per_gas > max_fee_per_gas {
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
