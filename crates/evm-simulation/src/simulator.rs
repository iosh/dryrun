use std::sync::Arc;

use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    network::Ethereum,
    providers::{DynProvider, Provider},
};
use tokio::runtime::Handle;

use crate::{
    EthereumChainSpec, EvmAnalysisDomain, EvmAnalysisView, EvmAnalyzerRegistry, EvmBlockContext,
    EvmChangeSet, EvmExecutionObserver, EvmExecutionOutcome, EvmInitializationError, EvmSimulation,
    EvmSimulationError, EvmSimulationLimits, EvmSimulationRequest, EvmTransactionExecutionResult,
    EvmTransactionExecutor, TypedTransaction, analysis::default_registry, resolve_block,
    state::EvmStateSource,
};

#[derive(Debug)]
pub struct EvmTransactionSimulator {
    provider: DynProvider<Ethereum>,
    chain_spec: Arc<EthereumChainSpec>,
    analyzers: Arc<EvmAnalyzerRegistry>,
    limits: EvmSimulationLimits,
}

impl Clone for EvmTransactionSimulator {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            chain_spec: Arc::clone(&self.chain_spec),
            analyzers: Arc::clone(&self.analyzers),
            limits: self.limits,
        }
    }
}

impl EvmTransactionSimulator {
    pub async fn ethereum_mainnet(
        provider: DynProvider<Ethereum>,
        limits: EvmSimulationLimits,
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

        let analyzers = default_registry(chain_spec.native_currency().clone());
        Ok(Self {
            provider,
            chain_spec: Arc::new(chain_spec),
            analyzers: Arc::new(analyzers),
            limits,
        })
    }
}

impl EvmTransactionSimulator {
    pub fn with_analyzers(mut self, analyzers: EvmAnalyzerRegistry) -> Self {
        self.analyzers = Arc::new(analyzers);
        self
    }

    pub fn with_analyzer(
        mut self,
        analyzer: impl simulation_core::analysis::Analyzer<EvmAnalysisDomain>,
    ) -> Result<Self, simulation_core::analysis::RegistryError> {
        self.analyzers = Arc::new(self.analyzers.with_analyzer(analyzer)?);
        Ok(self)
    }

    /// Simulates one transaction inside the caller's active Tokio runtime.
    pub async fn simulate(
        &self,
        request: EvmSimulationRequest,
    ) -> Result<EvmSimulation, EvmSimulationError> {
        simulation_core::simulation::simulate(EthereumBackend(self.clone()), request).await
    }
}

struct EthereumBackend(EvmTransactionSimulator);

impl simulation_core::simulation::SimulationBackend for EthereumBackend {
    type Request = EvmSimulationRequest;
    type Context = EvmBlockContext;
    type Transaction = TypedTransaction;
    type TransactionRequest = crate::TransactionRequest;
    type Rejection = crate::EvmTransactionRejection;
    type Prepared = Sealed<Header>;
    type Evidence = crate::EvmTransactionExecution;
    type Outcome = EvmExecutionOutcome;
    type ChangeSet = EvmChangeSet;
    type AnalysisError = crate::EvmAnalysisError;
    type Error = EvmSimulationError;

    async fn prepare(
        &self,
        request: Self::Request,
    ) -> Result<simulation_core::simulation::PreparationFor<Self>, Self::Error> {
        use simulation_core::{completion::Completion, simulation::Preparation};
        let EvmSimulationRequest { block, transaction } = request;
        let block = resolve_block(&self.0.provider, block).await?;
        let context = EvmBlockContext {
            number: block.number(),
            hash: block.hash(),
        };
        let transaction = match crate::complete_transaction(
            transaction,
            &self.0.provider,
            &block,
            &self.0.chain_spec,
        )
        .await?
        {
            Completion::Ready(transaction) => transaction,
            Completion::Rejected {
                transaction,
                rejection,
            } => {
                return Ok(Preparation::Rejected {
                    context: Some(context),
                    transaction,
                    rejection,
                });
            }
        };
        Ok(Preparation::Ready {
            context,
            transaction,
            execution: block,
        })
    }

    fn execute(
        &self,
        transaction: &Self::Transaction,
        block: Self::Prepared,
        runtime_handle: Handle,
    ) -> Result<simulation_core::simulation::Execution<Self::Evidence, Self::Rejection>, Self::Error>
    {
        use simulation_core::simulation::Execution;
        let checkpoint_filters = self.0.analyzers.checkpoint_filters().to_vec();
        let state_source =
            EvmStateSource::new(self.0.provider.clone(), runtime_handle, block.hash());
        let executor = EvmTransactionExecutor::new(
            state_source,
            block,
            &self.0.chain_spec,
            EvmExecutionObserver::new(checkpoint_filters, self.0.limits),
            self.0.limits,
        )?;
        match executor.execute(transaction)? {
            EvmTransactionExecutionResult::Executed(output) => {
                Ok(Execution::Executed(output.commit(transaction)?))
            }
            EvmTransactionExecutionResult::NotExecuted(rejection) => {
                Ok(Execution::Rejected(rejection))
            }
        }
    }

    fn is_success(&self, evidence: &Self::Evidence) -> bool {
        evidence.is_success()
    }

    fn analyze(
        &self,
        view: simulation_core::simulation::AnalysisView<
            '_,
            Self::Context,
            Self::Transaction,
            Self::Evidence,
        >,
    ) -> Result<Self::ChangeSet, Self::AnalysisError> {
        let execution = view.execution();
        execution.check_observation_limit()?;
        let view = EvmAnalysisView {
            context: view.context(),
            transaction: view.transaction(),
            execution,
        };
        let mut changes = self.0.analyzers.analyze(view)?;
        changes.load_metadata(execution.state().finalized())?;
        Ok(changes)
    }

    fn into_outcome(&self, evidence: Self::Evidence) -> Self::Outcome {
        evidence.into_outcome()
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use alloy::{
        network::Ethereum,
        providers::{DynProvider, Provider, RootProvider},
        rpc::client::RpcClient,
        transports::mock::Asserter,
    };

    use super::EvmTransactionSimulator;
    use crate::{EvmInitializationError, EvmSimulationLimits};

    #[test]
    fn returns_typed_initialization_errors() {
        let mismatch_asserter = Asserter::new();
        mismatch_asserter.push_success(&"0x5");
        let mismatch = block_on(EvmTransactionSimulator::ethereum_mainnet(
            mock_provider(mismatch_asserter),
            test_limits(),
        ))
        .expect_err("wrong chain id should reject initialization");
        assert!(matches!(
            mismatch,
            EvmInitializationError::ChainIdMismatch {
                expected: 1,
                actual: 5,
            }
        ));

        let failure_asserter = Asserter::new();
        failure_asserter.push_failure_msg("provider unavailable");
        let failure = block_on(EvmTransactionSimulator::ethereum_mainnet(
            mock_provider(failure_asserter),
            test_limits(),
        ))
        .expect_err("provider failure should reject initialization");
        assert!(matches!(
            failure,
            EvmInitializationError::ChainIdRequest { .. }
        ));
    }

    fn mock_provider(asserter: Asserter) -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(asserter)).erased()
    }

    fn test_limits() -> EvmSimulationLimits {
        EvmSimulationLimits::default()
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build")
            .block_on(future)
    }
}
