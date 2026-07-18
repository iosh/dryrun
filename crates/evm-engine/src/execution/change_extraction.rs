use crate::{
    Change, EvmEngineError, EvmExecutionStatus, EvmTransaction,
    execution::{ExecutionArtifacts, MainnetAlloyEvm, contract_reads::load_change_data},
    transaction_changes::{ChangeCandidate, ChangeDataRequests, build_changes, collect_candidates},
};

pub(super) fn collect_change_candidates(
    artifacts: &ExecutionArtifacts,
) -> Result<Vec<ChangeCandidate>, EvmEngineError> {
    if !matches!(artifacts.status, EvmExecutionStatus::Success) {
        return Ok(Vec::new());
    }

    collect_candidates(&artifacts.observations).map_err(|error| {
        EvmEngineError::analysis_failed(format!("transaction changes failed: {error}"))
    })
}

pub(super) fn build_transaction_changes<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    candidates: Vec<ChangeCandidate>,
    requests: ChangeDataRequests,
) -> Vec<Change> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let data = load_change_data(evm, transaction, chain_id, requests);

    build_changes(candidates, &data)
}
