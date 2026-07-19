use std::fmt::Display;

use alloy_primitives::Address;
use revm::state::EvmState;

use crate::{
    Change, EvmEngineError, EvmExecutionStatus, EvmTransaction,
    execution::{
        ExecutionArtifacts, MainnetAlloyEvm, contract_reads::load_change_data,
        fee_settlement::TransactionFeeSettlement,
    },
    transaction_changes::{
        self, ChangeCandidate, ChangeDataRequests, build_changes, collect_candidates,
    },
};

pub(super) fn collect_change_candidates(
    artifacts: &ExecutionArtifacts,
) -> Result<Vec<ChangeCandidate>, EvmEngineError> {
    if !matches!(artifacts.status, EvmExecutionStatus::Success) {
        return Ok(Vec::new());
    }

    collect_candidates(&artifacts.observations).map_err(map_transaction_changes_error)
}

pub(super) fn check_native_balances(
    state: &EvmState,
    candidates: &[ChangeCandidate],
    caller: Address,
    beneficiary: Address,
    fee_settlement: &TransactionFeeSettlement,
) -> Result<(), EvmEngineError> {
    transaction_changes::check_native_balances(
        state,
        candidates,
        caller,
        beneficiary,
        fee_settlement.gas_precharge,
        fee_settlement.caller_refund,
        fee_settlement.beneficiary_reward,
    )
    .map_err(map_transaction_changes_error)
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

fn map_transaction_changes_error(error: impl Display) -> EvmEngineError {
    EvmEngineError::analysis_failed(format!("transaction changes failed: {error}"))
}
