use std::sync::Arc;

use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    network::Ethereum,
    providers::{DynProvider, Provider},
};
use tokio::runtime::Handle;

use crate::{
    CompleteTransaction, EthereumChainSpec, EvmBlockContext, EvmExecutionObserver,
    EvmExecutionOutcome, EvmInitializationError, EvmSimulation, EvmSimulationError,
    EvmSimulationRequest, EvmTransactionExecution, EvmTransactionExecutor,
    changes::analyze_changes, create_database, map_executed_outcome, resolve_block,
};

#[derive(Debug, Clone)]
pub struct EvmTransactionSimulator {
    provider: DynProvider<Ethereum>,
    chain_spec: Arc<EthereumChainSpec>,
}

impl EvmTransactionSimulator {
    pub async fn ethereum_mainnet(
        provider: DynProvider<Ethereum>,
    ) -> Result<Self, EvmInitializationError> {
        let chain_spec = EthereumChainSpec::mainnet();
        let actual_chain_id = provider
            .get_chain_id()
            .await
            .map_err(EvmInitializationError::chain_id_request)?;

        if actual_chain_id != chain_spec.chain_id() {
            return Err(EvmInitializationError::ChainIdMismatch {
                expected: chain_spec.chain_id(),
                actual: actual_chain_id,
            });
        }

        Ok(Self {
            provider,
            chain_spec: Arc::new(chain_spec),
        })
    }

    pub async fn simulate(
        &self,
        request: EvmSimulationRequest,
    ) -> Result<EvmSimulation, EvmSimulationError> {
        let EvmSimulationRequest { block, transaction } = request;
        transaction.validate_requirements()?;
        let block = resolve_block(&self.provider, block).await?;
        let transaction =
            crate::complete_transaction(transaction, &self.provider, &block, &self.chain_spec)
                .await?;

        let provider = self.provider.clone();
        let chain_spec = Arc::clone(&self.chain_spec);
        let runtime_handle = Handle::current();

        tokio::task::spawn_blocking(move || {
            simulate_blocking(provider, runtime_handle, chain_spec, block, transaction)
        })
        .await
        .map_err(EvmSimulationError::execution_task)?
    }
}

fn simulate_blocking(
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    chain_spec: Arc<EthereumChainSpec>,
    block: Sealed<Header>,
    transaction: CompleteTransaction,
) -> Result<EvmSimulation, EvmSimulationError> {
    let context = EvmBlockContext {
        number: block.number(),
        hash: block.hash(),
    };
    let database = create_database(provider, runtime_handle, block.hash());
    let executor =
        EvmTransactionExecutor::new(database, block, &chain_spec, EvmExecutionObserver::new())?;

    let mut output = match executor.execute(&transaction)? {
        EvmTransactionExecution::Executed(output) => output,
        EvmTransactionExecution::NotExecuted(rejection) => {
            return Ok(EvmSimulation {
                context,
                transaction,
                execution: EvmExecutionOutcome::NotExecuted(rejection),
                changes: Vec::new(),
            });
        }
    };

    if !output.is_success() {
        let (engine_result, execution_result) = (*output).into_outcome_parts();
        let execution = map_executed_outcome(engine_result, &transaction, execution_result)?;
        return Ok(EvmSimulation {
            context,
            transaction,
            execution,
            changes: Vec::new(),
        });
    }

    let changes = analyze_changes(&mut output, &transaction, &chain_spec)?;

    let (engine_result, execution_result) = (*output).into_outcome_parts();
    let execution = map_executed_outcome(engine_result, &transaction, execution_result)?;

    Ok(EvmSimulation {
        context,
        transaction,
        execution,
        changes,
    })
}
