use std::sync::Arc;

use cfx_types::{Address, U256};
use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError, ConfluxTransactionBody, PreparedCoreSpaceSimulation,
    PreparedEspaceSimulation,
    config::ConfluxChainConfig,
    core_space::{
        CoreSpaceEpochRef, CoreSpaceExecution, CoreSpaceExecutionFailure,
        CoreSpaceExecutionFailureCode, CoreSpaceStateAnchor, CoreSpaceTransaction,
        CoreSpaceTransactionVariant, build_core_space_not_executed,
        build_core_space_transaction_input,
    },
    espace::{
        EspaceBlockRef, EspaceExecution, EspaceTransaction, build_espace_not_executed,
        build_espace_transaction_input, validate_espace_transaction,
    },
    execution::{DryRunTransactionInput, TransactionExecutionInput},
    preparation::{
        PreparedCoreSpaceSimulationState, PreparedEspaceSimulationState, ReadyCoreSpaceSimulation,
        ReadyEspaceSimulation, ResolvedCoreSpaceContext, ResolvedEspaceContext,
        resolve_core_space_execution_context, resolve_espace_execution_context,
    },
    state::{
        ConfluxBlockProvider, ConfluxStatePoint, ConfluxStateProvider, ConfluxTransactionProvider,
        CoreSpaceResourceEstimate, RemoteStateReader,
    },
};

pub struct ConfluxEngine {
    chain: ConfluxChainConfig,
    block_provider: Arc<dyn ConfluxBlockProvider>,
    state_provider: Arc<dyn ConfluxStateProvider>,
    transaction_provider: Arc<dyn ConfluxTransactionProvider>,
    runtime_handle: Handle,
}

impl ConfluxEngine {
    pub fn new<P>(chain: ConfluxChainConfig, provider: Arc<P>, runtime_handle: Handle) -> Self
    where
        P: ConfluxBlockProvider + ConfluxStateProvider + ConfluxTransactionProvider + 'static,
    {
        let block_provider: Arc<dyn ConfluxBlockProvider> = provider.clone();
        let state_provider: Arc<dyn ConfluxStateProvider> = provider.clone();
        let transaction_provider: Arc<dyn ConfluxTransactionProvider> = provider;

        Self {
            chain,
            block_provider,
            state_provider,
            transaction_provider,
            runtime_handle,
        }
    }

    pub async fn resolve_espace_context(
        &self,
        block: EspaceBlockRef,
    ) -> Result<ResolvedEspaceContext, ConfluxEngineError> {
        resolve_espace_execution_context(self.block_provider.as_ref(), &block).await
    }

    pub async fn espace_nonce(
        &self,
        context: &ResolvedEspaceContext,
        address: Address,
    ) -> Result<U256, ConfluxEngineError> {
        Ok(self
            .transaction_provider
            .eth_get_transaction_count(address, context.state_point.espace_block())
            .await?)
    }

    pub async fn eth_gas_price(&self) -> Result<U256, ConfluxEngineError> {
        Ok(self.transaction_provider.eth_gas_price().await?)
    }

    pub async fn eth_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxEngineError> {
        Ok(self
            .transaction_provider
            .eth_max_priority_fee_per_gas()
            .await?)
    }

    pub async fn eth_estimate_gas(
        &self,
        context: &ResolvedEspaceContext,
        transaction: &ConfluxTransactionBody,
    ) -> Result<U256, ConfluxEngineError> {
        Ok(self
            .transaction_provider
            .eth_estimate_gas(context.state_point.espace_block(), transaction)
            .await?)
    }

    pub async fn prepare_espace_transaction(
        &self,
        context: ResolvedEspaceContext,
        transaction: EspaceTransaction,
    ) -> Result<PreparedEspaceSimulation, ConfluxEngineError> {
        let gas_limit = transaction.gas_limit;
        let chain_id = self.chain.evm_chain_id;

        if let Err(failure) = validate_espace_transaction(&transaction, chain_id) {
            return Ok(PreparedEspaceSimulation {
                kind: PreparedEspaceSimulationState::Complete(Box::new(build_espace_not_executed(
                    chain_id,
                    context.simulated_block,
                    gas_limit,
                    failure,
                ))),
            });
        }

        let transaction = build_espace_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: context.block_context,
            transaction: DryRunTransactionInput::Espace(transaction),
        };
        let state_reader = self.prepare_state_reader(context.state_point).await?;

        Ok(PreparedEspaceSimulation {
            kind: PreparedEspaceSimulationState::Ready(Box::new(ReadyEspaceSimulation {
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
    ) -> Result<EspaceExecution, ConfluxEngineError> {
        crate::espace::simulation::simulate(prepared, &self.runtime_handle)
    }

    pub async fn resolve_core_space_context(
        &self,
        epoch: CoreSpaceEpochRef,
    ) -> Result<ResolvedCoreSpaceContext, ConfluxEngineError> {
        resolve_core_space_execution_context(self.block_provider.as_ref(), &epoch).await
    }

    pub async fn core_space_nonce(
        &self,
        context: &ResolvedCoreSpaceContext,
        address: Address,
    ) -> Result<U256, ConfluxEngineError> {
        Ok(self
            .transaction_provider
            .cfx_get_next_nonce(address, context.state_point.core_space_epoch())
            .await?)
    }

    pub async fn cfx_gas_price(&self) -> Result<U256, ConfluxEngineError> {
        Ok(self.transaction_provider.cfx_gas_price().await?)
    }

    pub async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxEngineError> {
        Ok(self
            .transaction_provider
            .cfx_max_priority_fee_per_gas()
            .await?)
    }

    pub async fn cfx_estimate_gas_and_collateral(
        &self,
        context: &ResolvedCoreSpaceContext,
        transaction: &ConfluxTransactionBody,
        epoch_height: u64,
        gas_limit: Option<U256>,
        storage_limit: Option<u64>,
    ) -> Result<CoreSpaceResourceEstimate, ConfluxEngineError> {
        Ok(self
            .transaction_provider
            .cfx_estimate_gas_and_collateral(
                context.state_point.core_space_epoch(),
                transaction,
                epoch_height,
                gas_limit,
                storage_limit,
            )
            .await?)
    }

    pub async fn prepare_core_space_transaction(
        &self,
        context: ResolvedCoreSpaceContext,
        transaction: CoreSpaceTransaction,
    ) -> Result<PreparedCoreSpaceSimulation, ConfluxEngineError> {
        let gas_limit = transaction.transaction.gas_limit;
        let chain_id = self.chain.core_space_chain_id;
        let state_anchor = CoreSpaceStateAnchor {
            epoch_number: context.state_point.anchor().epoch_number(),
            pivot_hash: context.state_point.anchor().pivot_hash(),
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
            block_context: context.block_context,
            transaction: DryRunTransactionInput::CoreSpace(transaction),
        };
        let state_reader = self.prepare_state_reader(context.state_point).await?;

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
        RemoteStateReader::prepare(state_point, Arc::clone(&self.state_provider))
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
    let transaction = &transaction.transaction.body;

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
        CoreSpaceTransactionVariant::Legacy { gas_price }
        | CoreSpaceTransactionVariant::AccessList { gas_price, .. } => {
            if gas_price.is_zero() {
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
