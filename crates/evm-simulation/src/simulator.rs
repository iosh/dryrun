use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    providers::RootProvider,
};
use alloy_chains::Chain;
use contract_standards::legacy::{metadata_requests, state_requirements, verify};
use simulation_changes::{
    ChangeMetadata, PositionedChange, into_enriched_changes, sort_changes_by_position,
};
use simulation_transaction::Transaction;
use tokio::runtime::Handle;

use crate::{
    EvmBlockAnchor, EvmExecutionError, EvmExecutionObserver, EvmNativeChangeError, EvmSimulation,
    EvmSimulationError, EvmStateSource, EvmTransactionExecutor, PreparedEvmSimulation,
    changes::{
        analyze_native_changes, collect_standard_candidates, load_standard_metadata,
        read_standard_state_values,
    },
    outcome::{build_execution, build_not_executed},
};

#[derive(Debug, Clone)]
pub struct EvmSimulator {
    provider: RootProvider,
    runtime_handle: Handle,
    chain_id: u64,
}

impl EvmSimulator {
    pub fn new(provider: RootProvider, runtime_handle: Handle) -> Self {
        Self {
            provider,
            runtime_handle,
            chain_id: Chain::mainnet().id(),
        }
    }

    pub fn simulate(
        &self,
        input: PreparedEvmSimulation,
    ) -> Result<EvmSimulation, EvmSimulationError> {
        let (block, transaction) = input.into_parts();
        simulate_prepared(
            &self.provider,
            &self.runtime_handle,
            self.chain_id,
            block,
            transaction,
        )
    }
}

fn simulate_prepared(
    provider: &RootProvider,
    runtime_handle: &Handle,
    chain_id: u64,
    block: Sealed<Header>,
    transaction: Transaction,
) -> Result<EvmSimulation, EvmSimulationError> {
    let state_source = EvmStateSource::new(
        provider.clone(),
        runtime_handle.clone(),
        EvmBlockAnchor::new(block.number(), block.hash()),
    );
    let executor = EvmTransactionExecutor::new(
        state_source,
        block.clone(),
        chain_id,
        EvmExecutionObserver::new(),
    )
    .map_err(map_execution_error)?;

    let mut output = match executor.execute(&transaction) {
        Ok(output) => output,
        Err(EvmExecutionError::NotExecuted(error)) => {
            return Ok(EvmSimulation::new(
                build_not_executed(chain_id, &block, &transaction, error),
                Vec::new(),
            ));
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
        return Ok(EvmSimulation::new(execution, Vec::new()));
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
        let metadata = ChangeMetadata::new(native_metadata(chain_id), standard_metadata);
        into_enriched_changes(positioned_changes, &metadata)
    };

    Ok(EvmSimulation::new(execution, changes))
}

fn native_metadata(chain_id: u64) -> crate::NativeMetadata {
    match chain_id {
        1 => crate::NativeMetadata {
            name: Some("Ether".to_string()),
            symbol: Some("ETH".to_string()),
            decimals: Some(18),
        },
        _ => crate::NativeMetadata::default(),
    }
}

fn map_native_change_error(error: EvmNativeChangeError) -> EvmSimulationError {
    match error {
        EvmNativeChangeError::TransitionUnavailable => EvmSimulationError::execution_error(
            "transaction execution transition was unavailable during native analysis",
        ),
        error => {
            EvmSimulationError::analysis_failed(format!("transaction changes failed: {error}"))
        }
    }
}

fn map_execution_error(error: EvmExecutionError) -> EvmSimulationError {
    match error {
        EvmExecutionError::UnsupportedChain(chain_id) => EvmSimulationError::not_supported(
            format!("only Ethereum mainnet is supported now, got chain_id={chain_id}"),
        ),
        EvmExecutionError::UnsupportedHardfork(hardfork) => EvmSimulationError::not_ready(format!(
            "hardfork {hardfork} is not mapped to revm::SpecId yet"
        )),
        EvmExecutionError::BlockContext(details) => {
            EvmSimulationError::block_context_error(details)
        }
        EvmExecutionError::StateAccess(details) => EvmSimulationError::state_access_error(details),
        EvmExecutionError::Execution(details) => EvmSimulationError::execution_error(details),
        EvmExecutionError::FeeSettlement => EvmSimulationError::execution_error(
            "transaction fee settlement arithmetic was inconsistent",
        ),
        EvmExecutionError::TransitionAlreadyApplied => {
            EvmSimulationError::execution_error("transaction transition has already been applied")
        }
        EvmExecutionError::TransitionNotApplicable => EvmSimulationError::execution_error(
            "transaction transition is only applicable to a successful execution",
        ),
        EvmExecutionError::NotExecuted(_) => EvmSimulationError::internal(
            "invalid transaction was unexpectedly treated as an outer error",
        ),
    }
}
