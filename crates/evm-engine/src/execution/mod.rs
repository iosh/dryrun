mod metadata_reads;
mod outcome;
mod read_call;
mod token_state_reads;

use crate::{
    EvmEngineError, EvmExecutionInput, EvmSimulation,
    changes::{
        PositionedChange, collect_contract_candidates, into_enriched_changes,
        sort_changes_by_position,
    },
    execution::{
        metadata_reads::load_change_metadata,
        outcome::{build_execution, build_not_executed},
        token_state_reads::read_token_state_values,
    },
};
use alloy::providers::RootProvider;
use evm_simulation::{
    EvmBlockAnchor, EvmExecutionError, EvmExecutionObserver, EvmNativeChangeError, EvmStateSource,
    EvmTransactionExecutor, MainnetEvmDatabase, analyze_native_changes,
};
use revm::{
    Context,
    context::{BlockEnv, CfgEnv, TxEnv},
};
use tokio::runtime::Handle;

use contract_standards::{MetadataRequests, state_requirements, verify};

pub(super) type MainnetEvmWithDb<DB, INSP = ()> =
    revm::MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB>, INSP>;
pub(super) type MainnetAlloyEvm<INSP = ()> = MainnetEvmWithDb<MainnetEvmDatabase, INSP>;

pub(crate) fn simulate_execution(
    provider: &RootProvider,
    runtime_handle: &Handle,
    chain_id: u64,
    input: EvmExecutionInput,
) -> Result<EvmSimulation, EvmEngineError> {
    let EvmExecutionInput { block, transaction } = input;
    let state_source = EvmStateSource::new(
        provider.clone(),
        runtime_handle.clone(),
        EvmBlockAnchor::new(block.number(), block.hash()),
    );
    let executor = EvmTransactionExecutor::new(
        state_source,
        block.cloned_header(),
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

    let candidates = collect_contract_candidates(&output.observations())?;
    let requirements = state_requirements(&candidates);

    let mut positioned_changes =
        analyze_native_changes(&output).map_err(map_native_change_error)?;

    let before_token_state =
        read_token_state_values(output.evm_mut(), &transaction, chain_id, &requirements)?;

    output.apply_transition().map_err(map_execution_error)?;

    let after_token_state =
        read_token_state_values(output.evm_mut(), &transaction, chain_id, &requirements)?;
    let standard_changes = verify(&candidates, &before_token_state, &after_token_state)?;
    let metadata_requests = MetadataRequests::from_changes(&standard_changes);
    positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));

    let changes = if positioned_changes.is_empty() {
        Vec::new()
    } else {
        sort_changes_by_position(&mut positioned_changes);
        let metadata =
            load_change_metadata(output.evm_mut(), &transaction, chain_id, metadata_requests)?;
        into_enriched_changes(positioned_changes, &metadata)
    };

    Ok(EvmSimulation::new(execution, changes))
}

fn map_native_change_error(error: EvmNativeChangeError) -> EvmEngineError {
    match error {
        EvmNativeChangeError::TransitionUnavailable => EvmEngineError::engine_execution_error(
            "transaction execution transition was unavailable during native analysis",
        ),
        error => EvmEngineError::analysis_failed(format!("transaction changes failed: {error}")),
    }
}

fn map_execution_error(error: EvmExecutionError) -> EvmEngineError {
    match error {
        EvmExecutionError::UnsupportedChain(chain_id) => EvmEngineError::not_supported(format!(
            "only Ethereum mainnet is supported now, got chain_id={chain_id}"
        )),
        EvmExecutionError::UnsupportedHardfork(hardfork) => EvmEngineError::not_ready(format!(
            "hardfork {hardfork} is not mapped to revm::SpecId yet"
        )),
        EvmExecutionError::BlockContext(details) => EvmEngineError::block_context_error(details),
        EvmExecutionError::StateAccess(details) => EvmEngineError::state_access_error(details),
        EvmExecutionError::Execution(details) => EvmEngineError::engine_execution_error(details),
        EvmExecutionError::FeeSettlement => EvmEngineError::engine_execution_error(
            "transaction fee settlement arithmetic was inconsistent",
        ),
        EvmExecutionError::TransitionAlreadyApplied => EvmEngineError::engine_execution_error(
            "transaction transition has already been applied",
        ),
        EvmExecutionError::TransitionNotApplicable => EvmEngineError::engine_execution_error(
            "transaction transition is only applicable to a successful execution",
        ),
        EvmExecutionError::NotExecuted(_) => EvmEngineError::internal(
            "invalid transaction was unexpectedly treated as an outer error",
        ),
    }
}
