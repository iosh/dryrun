use crate::{
    ConfluxSimulationBackend, ConfluxSimulationError, PreparedCoreSpaceSimulation,
    execution::{DryRunTransactionInput, TransactionExecutionInput},
    preparation::{
        PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation, complete_core_space_transaction,
    },
};

use super::{
    CoreSpaceBlockSelector, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
    CoreSpaceTransaction, CoreSpaceTransactionRequest, CoreSpaceTransactionVariant,
    ResolvedCoreSpaceContext, build_core_space_not_executed, build_core_space_transaction_input,
    prepare_storage_payer, resolve_core_space_context, validate_core_space_transaction_network,
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
        request: CoreSpaceTransactionRequest,
        storage_limit: Option<u64>,
        epoch_height: Option<u64>,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxSimulationError> {
        validate_core_space_transaction_network(
            &request,
            self.backend.core_space_address_network(),
        )?;
        let context = resolve_core_space_context(self.backend.provider(), selector).await?;
        let transaction = complete_core_space_transaction(
            self.backend.provider(),
            &context,
            request,
            storage_limit,
            epoch_height,
        )
        .await?;
        self.prepare_completed_transaction(context, transaction)
            .await
    }

    async fn prepare_completed_transaction(
        &self,
        context: ResolvedCoreSpaceContext,
        transaction: CoreSpaceTransaction,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxSimulationError> {
        let gas_limit = transaction.gas_limit;
        let chain_id = self.backend.chain_spec().core_space_chain_id();
        let public_context = context.public_context;

        if let Err(failure) = validate_core_space_transaction(&transaction, chain_id) {
            return Ok(PreparedCoreSpaceSimulation {
                state: PreparedCoreSpaceSimulationState::Finished(Box::new(
                    build_core_space_not_executed(chain_id, public_context, gas_limit, failure),
                )),
            });
        }

        let storage_payer =
            prepare_storage_payer(self.backend.provider(), context.state_anchor, &transaction)
                .await?;
        let transaction = build_core_space_transaction_input(transaction, chain_id);
        let execution_input = TransactionExecutionInput {
            block_context: context.execution_block_context,
            transaction: DryRunTransactionInput::CoreSpace(transaction),
        };
        let state_source = self
            .backend
            .prepare_state_source(context.state_anchor)
            .await?;

        Ok(PreparedCoreSpaceSimulation {
            state: PreparedCoreSpaceSimulationState::Ready(Box::new(ReadyCoreSpaceSimulation {
                backend: self.backend.clone(),
                chain_id,
                public_context,
                gas_limit,
                storage_payer,
                execution_input,
                state_source,
            })),
        })
    }
}

fn validate_core_space_transaction(
    transaction: &CoreSpaceTransaction,
    expected_chain_id: u32,
) -> Result<(), CoreSpaceExecutionFailure> {
    if transaction.chain_id != u64::from(expected_chain_id) {
        return Err(CoreSpaceExecutionFailure {
            code: CoreSpaceExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match simulation chain id {}",
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
