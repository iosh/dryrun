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
    EvmSimulationLimits, EvmSimulationRequest, EvmTransactionExecutionResult,
    EvmTransactionExecutor,
    changeset::{
        CombinedEvmChangeResolver, EvmChangeResolver, EvmChangeSet, EvmChanges,
        StandardEvmChangeResolver,
    },
    map_executed_outcome, resolve_block,
    state::EvmStateSource,
};

#[derive(Debug)]
pub struct EvmTransactionSimulator<R = StandardEvmChangeResolver> {
    provider: DynProvider<Ethereum>,
    chain_spec: Arc<EthereumChainSpec>,
    resolver: Arc<R>,
    limits: EvmSimulationLimits,
}

impl<R> Clone for EvmTransactionSimulator<R> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            chain_spec: Arc::clone(&self.chain_spec),
            resolver: Arc::clone(&self.resolver),
            limits: self.limits.clone(),
        }
    }
}

impl EvmTransactionSimulator<StandardEvmChangeResolver> {
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

        let resolver = StandardEvmChangeResolver::with_wrapped_native_token(
            chain_spec.native_currency().clone(),
            chain_spec.wrapped_native_token_address(),
        );
        Ok(Self {
            provider,
            chain_spec: Arc::new(chain_spec),
            resolver: Arc::new(resolver),
            limits: EvmSimulationLimits::default(),
        })
    }
}

impl<R> EvmTransactionSimulator<R> {
    pub fn with_change_resolver<N>(self, resolver: N) -> EvmTransactionSimulator<N>
    where
        N: EvmChangeResolver,
    {
        EvmTransactionSimulator {
            provider: self.provider,
            chain_spec: self.chain_spec,
            resolver: Arc::new(resolver),
            limits: self.limits,
        }
    }

    pub fn with_additional_change_resolver<N>(
        self,
        resolver: N,
    ) -> EvmTransactionSimulator<CombinedEvmChangeResolver<R, N>>
    where
        R: EvmChangeResolver,
        N: EvmChangeResolver,
    {
        EvmTransactionSimulator {
            provider: self.provider,
            chain_spec: self.chain_spec,
            resolver: Arc::new(CombinedEvmChangeResolver::from_shared(
                self.resolver,
                resolver,
            )),
            limits: self.limits,
        }
    }

    pub fn with_limits(mut self, limits: EvmSimulationLimits) -> Self {
        self.limits = limits;
        self
    }
}

impl<R> EvmTransactionSimulator<R>
where
    R: EvmChangeResolver,
{
    /// Simulates one transaction and resolves its verified wallet semantic changes.
    ///
    /// The returned future must be polled inside an active Tokio runtime.
    pub async fn simulate(
        &self,
        request: EvmSimulationRequest,
    ) -> Result<EvmSimulation, EvmSimulationError> {
        self.run_simulation(request, simulate_verified_changes_blocking::<R>)
            .await
    }

    async fn run_simulation<T>(
        &self,
        request: EvmSimulationRequest,
        simulate_blocking: BlockingSimulation<R, T>,
    ) -> Result<T, EvmSimulationError>
    where
        T: Send + 'static,
    {
        let EvmSimulationRequest { block, transaction } = request;
        let runtime_handle =
            Handle::try_current().map_err(|_| EvmSimulationError::RuntimeUnavailable)?;
        let block = resolve_block(&self.provider, block).await?;
        let transaction =
            crate::complete_transaction(transaction, &self.provider, &block, &self.chain_spec)
                .await?;

        let provider = self.provider.clone();
        let chain_spec = Arc::clone(&self.chain_spec);
        let resolver = Arc::clone(&self.resolver);
        let limits = self.limits.clone();
        let blocking_runtime_handle = runtime_handle.clone();

        runtime_handle
            .spawn_blocking(move || {
                simulate_blocking(BlockingSimulationInput {
                    provider,
                    runtime_handle: blocking_runtime_handle,
                    chain_spec,
                    resolver,
                    limits,
                    block,
                    transaction,
                })
            })
            .await
            .map_err(EvmSimulationError::execution_task)?
    }
}

type BlockingSimulation<R, T> = fn(BlockingSimulationInput<R>) -> Result<T, EvmSimulationError>;

struct BlockingSimulationInput<R> {
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    chain_spec: Arc<EthereumChainSpec>,
    resolver: Arc<R>,
    limits: EvmSimulationLimits,
    block: Sealed<Header>,
    transaction: CompleteTransaction,
}

fn simulate_verified_changes_blocking<R>(
    input: BlockingSimulationInput<R>,
) -> Result<EvmSimulation, EvmSimulationError>
where
    R: EvmChangeResolver,
{
    let BlockingSimulationInput {
        provider,
        runtime_handle,
        chain_spec,
        resolver,
        limits,
        block,
        transaction,
    } = input;
    let context = EvmBlockContext {
        number: block.number(),
        hash: block.hash(),
    };
    let executor = create_executor(provider, runtime_handle, block, &chain_spec, limits)?;
    let (output, state_views) = match executor.execute(&transaction)? {
        EvmTransactionExecutionResult::Executed(output) => output.commit()?,
        EvmTransactionExecutionResult::NotExecuted(rejection) => {
            return Ok(EvmSimulation {
                context,
                transaction,
                execution: EvmExecutionOutcome::NotExecuted(rejection),
                changes: EvmChanges::Complete(EvmChangeSet::default()),
            });
        }
    };

    let changes = EvmChanges::from(resolver.resolve(&output, &state_views));
    let (engine_result, execution_result) = output.into_outcome_parts();
    let execution = map_executed_outcome(engine_result, &transaction, execution_result)?;

    Ok(EvmSimulation {
        context,
        transaction,
        execution,
        changes,
    })
}

fn create_executor(
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    block: Sealed<Header>,
    chain_spec: &EthereumChainSpec,
    limits: EvmSimulationLimits,
) -> Result<EvmTransactionExecutor<EvmExecutionObserver>, EvmSimulationError> {
    let block_hash = block.hash();
    let state_source = EvmStateSource::new(provider, runtime_handle, block_hash);
    EvmTransactionExecutor::new(
        state_source,
        block,
        chain_spec,
        EvmExecutionObserver::with_limits(
            chain_spec.wrapped_native_token_address(),
            limits.clone(),
        ),
        limits,
    )
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
    use crate::EvmInitializationError;

    #[test]
    fn returns_typed_initialization_errors() {
        let mismatch_asserter = Asserter::new();
        mismatch_asserter.push_success(&"0x5");
        let mismatch = block_on(EvmTransactionSimulator::ethereum_mainnet(mock_provider(
            mismatch_asserter,
        )))
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
        let failure = block_on(EvmTransactionSimulator::ethereum_mainnet(mock_provider(
            failure_asserter,
        )))
        .expect_err("provider failure should reject initialization");
        assert!(matches!(
            failure,
            EvmInitializationError::ChainIdRequest { .. }
        ));
    }

    fn mock_provider(asserter: Asserter) -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(asserter)).erased()
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build")
            .block_on(future)
    }
}
