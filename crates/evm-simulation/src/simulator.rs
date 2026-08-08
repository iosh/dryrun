use alloy::{
    consensus::{Header, Sealed},
    network::Ethereum,
    providers::{DynProvider, Provider},
};
use contract_standards::legacy::{metadata_requests, state_requirements, verify};
use simulation_changes::{
    ChangeMetadata, PositionedChange, into_enriched_changes, sort_changes_by_position,
};
use tokio::runtime::Handle;

use crate::{
    CompleteTransaction, EthereumChainSpec, EvmExecutionError, EvmExecutionObserver,
    EvmInitializationError, EvmNativeChangeError, EvmSimulation, EvmSimulationError,
    EvmSimulationRequest, EvmTransactionExecutor,
    changes::{
        analyze_native_changes, collect_standard_candidates, load_standard_metadata,
        read_standard_state_values,
    },
    create_database,
    outcome::{build_execution, build_not_executed},
    resolve_block,
};

#[derive(Debug, Clone)]
pub struct EvmTransactionSimulator {
    provider: DynProvider<Ethereum>,
    chain_spec: EthereumChainSpec,
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
            chain_spec,
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
            crate::complete_transaction(transaction, &self.provider, &block, self.chain_spec)
                .await?;

        let provider = self.provider.clone();
        let chain_spec = self.chain_spec;
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
    chain_spec: EthereumChainSpec,
    block: Sealed<Header>,
    transaction: CompleteTransaction,
) -> Result<EvmSimulation, EvmSimulationError> {
    let chain_id = chain_spec.chain_id();
    let database = create_database(provider, runtime_handle, block.hash());
    let executor = EvmTransactionExecutor::new(
        database,
        block.clone(),
        chain_spec,
        EvmExecutionObserver::new(),
    )
    .map_err(map_execution_error)?;

    let mut output = match executor.execute(&transaction) {
        Ok(output) => output,
        Err(EvmExecutionError::NotExecuted(error)) => {
            let execution = build_not_executed(chain_id, &block, &transaction, error);
            return Ok(EvmSimulation::new(transaction, execution, Vec::new()));
        }
        Err(error) => return Err(map_execution_error(error)),
    };

    let execution = build_execution(
        output.result().clone(),
        chain_id,
        &block,
        output.fee_settlement(),
    );
    if !output.result().is_success() {
        return Ok(EvmSimulation::new(transaction, execution, Vec::new()));
    }

    let candidates = collect_standard_candidates(&output.observations())?;
    let requirements = state_requirements(&candidates);
    let mut positioned_changes =
        analyze_native_changes(&output).map_err(map_native_change_error)?;

    let before_token_state =
        read_standard_state_values(output.evm_mut(), &transaction, chain_id, &requirements)?;

    output.apply_transition().map_err(map_execution_error)?;

    let after_token_state =
        read_standard_state_values(output.evm_mut(), &transaction, chain_id, &requirements)?;
    let standard_changes = verify(&candidates, &before_token_state, &after_token_state)?;
    let metadata_requests = metadata_requests(&standard_changes);
    positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));

    let changes = if positioned_changes.is_empty() {
        Vec::new()
    } else {
        sort_changes_by_position(&mut positioned_changes);
        let standard_metadata =
            load_standard_metadata(output.evm_mut(), &transaction, chain_id, metadata_requests)?;
        let metadata = ChangeMetadata::new(native_metadata(chain_spec), standard_metadata);
        into_enriched_changes(positioned_changes, &metadata)
    };

    Ok(EvmSimulation::new(transaction, execution, changes))
}

fn native_metadata(chain_spec: EthereumChainSpec) -> crate::NativeMetadata {
    let native_currency = chain_spec.native_currency();
    crate::NativeMetadata {
        name: Some(native_currency.name.to_string()),
        symbol: Some(native_currency.symbol.to_string()),
        decimals: Some(native_currency.decimals),
    }
}

fn map_native_change_error(error: EvmNativeChangeError) -> EvmSimulationError {
    match error {
        EvmNativeChangeError::TransitionUnavailable => EvmSimulationError::execution(
            "transaction execution transition was unavailable during native analysis",
        ),
        error => EvmSimulationError::changes(error.to_string()),
    }
}

fn map_execution_error(error: EvmExecutionError) -> EvmSimulationError {
    match error {
        EvmExecutionError::UnsupportedHardfork(error) => {
            EvmSimulationError::not_ready(error.to_string())
        }
        error @ EvmExecutionError::Eip4844SettlementNotReady => {
            EvmSimulationError::not_ready(error.to_string())
        }
        error @ EvmExecutionError::Eip7702SettlementNotReady => {
            EvmSimulationError::not_ready(error.to_string())
        }
        EvmExecutionError::BlockContext(details) => EvmSimulationError::block_context(details),
        EvmExecutionError::StateAccess(details) => EvmSimulationError::state_access(details),
        EvmExecutionError::Execution(details) => EvmSimulationError::execution(details),
        EvmExecutionError::FeeSettlement => {
            EvmSimulationError::execution("transaction fee settlement arithmetic was inconsistent")
        }
        EvmExecutionError::TransitionAlreadyApplied => {
            EvmSimulationError::execution("transaction transition has already been applied")
        }
        EvmExecutionError::TransitionNotApplicable => EvmSimulationError::execution(
            "transaction transition is only applicable to a successful execution",
        ),
        EvmExecutionError::NotExecuted(_) => EvmSimulationError::internal(
            "invalid transaction was unexpectedly treated as an outer error",
        ),
    }
}
