use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationBackend,
    chain_spec::CoreSpaceTransactionValidationRules,
    execution::{next_execution_block_number, next_execution_epoch_height},
    state::ConfluxStateSource,
};

use super::{
    CoreSpaceCompleteTransaction, CoreSpaceCompleteTransactionVariant, CoreSpaceExecutionError,
    CoreSpaceSimulation, CoreSpaceSimulationError, CoreSpaceSimulationRequest,
    CoreSpaceStateAccessError, CoreSpaceTransactionRejection, build_core_space_execution,
    build_core_space_not_executed, complete_transaction, resolve_core_space_context,
    resolve_storage_sponsorship, session::CoreSpaceExecutionSession,
};

#[derive(Clone)]
pub struct CoreSpaceTransactionSimulator {
    backend: ConfluxSimulationBackend,
}

impl CoreSpaceTransactionSimulator {
    pub const fn new(backend: ConfluxSimulationBackend) -> Self {
        Self { backend }
    }

    /// Simulates one Core Space transaction inside the caller's active Tokio runtime.
    pub async fn simulate(
        &self,
        request: CoreSpaceSimulationRequest,
    ) -> Result<CoreSpaceSimulation, CoreSpaceSimulationError> {
        let CoreSpaceSimulationRequest { block, transaction } = request;
        transaction.validate_network(self.backend.core_space_address_network())?;
        let runtime_handle =
            Handle::try_current().map_err(|_| CoreSpaceSimulationError::RuntimeUnavailable)?;
        let context = resolve_core_space_context(self.backend.provider(), block).await?;
        let chain_id = self.backend.chain_spec().core_space_chain_id();
        let transaction =
            complete_transaction(transaction, self.backend.provider(), &context, chain_id).await?;
        let gas_limit = transaction.gas_limit;
        let execution_block_number =
            next_execution_block_number(context.execution_block_context.pivot_block_number)
                .map_err(|source| CoreSpaceExecutionError::Context { source })?;
        let execution_epoch_height =
            next_execution_epoch_height(context.execution_block_context.pivot_epoch_height)
                .map_err(|source| CoreSpaceExecutionError::Context { source })?;
        let rules = self
            .backend
            .chain_spec()
            .core_space_transaction_validation_rules(
                execution_block_number,
                execution_epoch_height,
            );

        if let Some(rejection) = classify_transaction_rejection(&transaction, chain_id, rules) {
            let execution = build_core_space_not_executed(
                chain_id,
                context.public_context,
                gas_limit,
                rejection,
            );
            return Ok(CoreSpaceSimulation::new(
                context.public_context,
                transaction,
                execution,
                Vec::new(),
            ));
        }

        let storage_sponsorship = resolve_storage_sponsorship(
            self.backend.provider(),
            context.state_anchor,
            &transaction,
        )
        .await?;
        let state_source =
            ConfluxStateSource::prepare(context.state_anchor, self.backend.provider().clone())
                .await
                .map_err(|source| {
                    CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Preparation {
                        source,
                    })
                })?;
        let backend = self.backend.clone();
        let blocking_runtime_handle = runtime_handle.clone();

        runtime_handle
            .spawn_blocking(move || {
                simulate_blocking(
                    backend,
                    blocking_runtime_handle,
                    chain_id,
                    context,
                    transaction,
                    storage_sponsorship,
                    state_source,
                )
            })
            .await
            .map_err(CoreSpaceSimulationError::execution_task)?
    }
}

fn simulate_blocking(
    backend: ConfluxSimulationBackend,
    runtime_handle: Handle,
    chain_id: u32,
    context: super::ResolvedCoreSpaceContext,
    transaction: CoreSpaceCompleteTransaction,
    storage_sponsorship: super::ResolvedStorageSponsorship,
    state_source: ConfluxStateSource,
) -> Result<CoreSpaceSimulation, CoreSpaceSimulationError> {
    let session = CoreSpaceExecutionSession::new(&backend, state_source, runtime_handle)?;
    let session_result = session.execute(
        &transaction,
        context.execution_block_context,
        storage_sponsorship,
    )?;
    let execution = build_core_space_execution(
        chain_id,
        context.public_context,
        transaction.gas_limit,
        session_result.outcome,
    );

    Ok(CoreSpaceSimulation::new(
        context.public_context,
        transaction,
        execution,
        session_result.changes,
    ))
}

fn classify_transaction_rejection(
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
