use std::fmt::Display;

use alloy_primitives::Address;
use revm::state::EvmState;

use crate::{
    Change, EvmEngineError, EvmExecutionStatus, EvmTransaction,
    execution::{
        ExecutionArtifacts, MainnetAlloyEvm, contract_reads::load_change_metadata,
        fee_settlement::TransactionFeeSettlement,
    },
    transaction_changes::{
        self, ChangeCandidate, PositionedChange, TokenStateKeys, TokenStateValues, build_changes,
        collect_candidates, collect_change_metadata_requests, sort_changes_by_position,
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
) -> Result<Vec<PositionedChange>, EvmEngineError> {
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

pub(super) fn check_erc20_changes(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, EvmEngineError> {
    transaction_changes::check_erc20_changes(candidates, keys, before, after)
        .map_err(map_transaction_changes_error)
}

pub(super) fn check_erc721_changes(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, EvmEngineError> {
    transaction_changes::check_erc721_changes(candidates, keys, before, after)
        .map_err(map_transaction_changes_error)
}

pub(super) fn check_erc1155_movements(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, EvmEngineError> {
    transaction_changes::check_erc1155_movements(candidates, keys, before, after)
        .map_err(map_transaction_changes_error)
}

pub(super) fn check_operator_approvals(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, EvmEngineError> {
    transaction_changes::check_operator_approvals(candidates, keys, before, after)
        .map_err(map_transaction_changes_error)
}

pub(super) fn check_token_contracts(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), EvmEngineError> {
    transaction_changes::check_token_contracts(candidates, keys, before, after)
        .map_err(map_transaction_changes_error)
}

pub(super) fn build_transaction_changes<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    mut positioned_changes: Vec<PositionedChange>,
) -> Vec<Change> {
    if positioned_changes.is_empty() {
        return Vec::new();
    }

    sort_changes_by_position(&mut positioned_changes);
    let requests = collect_change_metadata_requests(&positioned_changes);
    let metadata = load_change_metadata(evm, transaction, chain_id, requests);

    build_changes(positioned_changes, &metadata)
}

fn map_transaction_changes_error(error: impl Display) -> EvmEngineError {
    EvmEngineError::analysis_failed(format!("transaction changes failed: {error}"))
}
