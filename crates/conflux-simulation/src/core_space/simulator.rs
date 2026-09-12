use std::sync::Arc;

use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationBackend,
    chain_spec::CoreSpaceTransactionValidationRules,
    execution::{next_execution_block_number, next_execution_epoch_height},
    state::ConfluxStateSource,
};

use super::{
    CoreSpaceChanges, CoreSpaceCompleteTransaction, CoreSpaceExecutionError,
    CoreSpaceExecutionOutcome, CoreSpaceSimulation, CoreSpaceSimulationError,
    CoreSpaceSimulationRequest, CoreSpaceStateAccessError, CoreSpaceTransactionRejection,
    complete_transaction, resolve_core_space_context, resolve_storage_sponsorship,
    session::CoreSpaceExecutionSession,
};

pub struct CoreSpaceTransactionSimulator<R = super::DefaultCoreSpaceChangeRules> {
    backend: ConfluxSimulationBackend,
    change_rules: Arc<R>,
}

impl<R> Clone for CoreSpaceTransactionSimulator<R> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            change_rules: Arc::clone(&self.change_rules),
        }
    }
}

impl CoreSpaceTransactionSimulator<super::DefaultCoreSpaceChangeRules> {
    pub fn new(backend: ConfluxSimulationBackend) -> Self {
        let change_rules = super::DefaultCoreSpaceChangeRules::new(
            backend.chain_spec().core_space_native_currency().clone(),
        );
        Self {
            backend,
            change_rules: Arc::new(change_rules),
        }
    }
}

impl<R> CoreSpaceTransactionSimulator<R> {
    pub fn with_change_rules<N>(self, change_rules: N) -> CoreSpaceTransactionSimulator<N>
    where
        N: super::CoreSpaceChangeRules,
    {
        CoreSpaceTransactionSimulator {
            backend: self.backend,
            change_rules: Arc::new(change_rules),
        }
    }

    pub fn with_additional_change_rules<N>(
        self,
        change_rules: N,
    ) -> CoreSpaceTransactionSimulator<super::CombinedCoreSpaceChangeRules<R, N>>
    where
        R: super::CoreSpaceChangeRules,
        N: super::CoreSpaceChangeRules,
    {
        CoreSpaceTransactionSimulator {
            backend: self.backend,
            change_rules: Arc::new(super::CombinedCoreSpaceChangeRules::from_shared(
                self.change_rules,
                change_rules,
            )),
        }
    }
}

impl<R> CoreSpaceTransactionSimulator<R>
where
    R: super::CoreSpaceChangeRules,
{
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

        if let Some(rejection) = validate_transaction_for_execution(&transaction, chain_id, rules) {
            return Ok(CoreSpaceSimulation::new(
                context.public_context,
                transaction,
                CoreSpaceExecutionOutcome::NotExecuted(rejection),
                CoreSpaceChanges::Complete(super::CoreSpaceChangeSet::default()),
            ));
        }

        let execution_spec = self
            .backend
            .chain_spec()
            .common_params()
            .spec(execution_block_number, execution_epoch_height);
        let storage_sponsorship = if execution_spec.cip78a || execution_spec.cip78b {
            Some(
                resolve_storage_sponsorship(
                    self.backend.provider(),
                    context.state_anchor,
                    &transaction,
                )
                .await?,
            )
        } else {
            None
        };
        let state_source =
            ConfluxStateSource::prepare(context.state_anchor, self.backend.provider().clone())
                .await
                .map_err(|source| {
                    CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Preparation {
                        source,
                    })
                })?;
        let backend = self.backend.clone();
        let change_rules = Arc::clone(&self.change_rules);
        let blocking_runtime_handle = runtime_handle.clone();

        runtime_handle
            .spawn_blocking(move || {
                simulate_blocking(
                    backend,
                    blocking_runtime_handle,
                    context,
                    transaction,
                    storage_sponsorship,
                    state_source,
                    change_rules,
                )
            })
            .await
            .map_err(CoreSpaceSimulationError::execution_task)?
    }
}

fn simulate_blocking<R>(
    backend: ConfluxSimulationBackend,
    runtime_handle: Handle,
    context: super::ResolvedCoreSpaceContext,
    transaction: CoreSpaceCompleteTransaction,
    storage_sponsorship: Option<super::ResolvedStorageSponsorship>,
    state_source: ConfluxStateSource,
    change_rules: Arc<R>,
) -> Result<CoreSpaceSimulation, CoreSpaceSimulationError>
where
    R: super::CoreSpaceChangeRules,
{
    let session = CoreSpaceExecutionSession::new(&backend, state_source, runtime_handle)?;
    let session_result = session.execute(
        &transaction,
        context.execution_block_context,
        storage_sponsorship,
        change_rules.as_ref(),
    )?;
    Ok(CoreSpaceSimulation::new(
        context.public_context,
        transaction,
        session_result.outcome,
        session_result.changes,
    ))
}

fn validate_transaction_for_execution(
    transaction: &CoreSpaceCompleteTransaction,
    expected_chain_id: u32,
    rules: CoreSpaceTransactionValidationRules,
) -> Option<CoreSpaceTransactionRejection> {
    let common = transaction.common();
    if common.chain_id != expected_chain_id {
        return Some(CoreSpaceTransactionRejection::InvalidChainId {
            transaction_chain_id: common.chain_id,
            expected_chain_id,
        });
    }

    if !rules.typed_transactions_active {
        match transaction {
            CoreSpaceCompleteTransaction::Cip155 { .. } => {}
            CoreSpaceCompleteTransaction::Cip2930 { .. } => {
                return Some(CoreSpaceTransactionRejection::Cip2930NotActivated);
            }
            CoreSpaceCompleteTransaction::Cip1559 { .. } => {
                return Some(CoreSpaceTransactionRejection::Cip1559NotActivated);
            }
        }
    }

    match transaction {
        CoreSpaceCompleteTransaction::Cip155 { gas_price, .. }
        | CoreSpaceCompleteTransaction::Cip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Some(CoreSpaceTransactionRejection::ZeroGasPrice);
            }
        }
        CoreSpaceCompleteTransaction::Cip1559 {
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
