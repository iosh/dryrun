use alloy_primitives::{Address, Bytes, U256};
use revm::{
    Database, ExecuteEvm,
    context::TxEnv,
    context_interface::{
        result::{EVMError, ExecutionResult, HaltReason},
        transaction::{AccessList as RevmAccessList, TransactionType},
    },
    handler::EvmTr,
    primitives::TxKind,
};

use crate::{CompleteTransaction, execution::MainnetEvmWithDb};

const READ_CALL_GAS_LIMIT: u64 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReadCallOutcome {
    Success(Bytes),
    Revert(Bytes),
    Halt(HaltReason),
}

pub(super) fn with_read_call_context<DB, INSP, T>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    operation: impl FnOnce(&mut MainnetEvmWithDb<DB, INSP>) -> T,
) -> T
where
    DB: Database,
{
    let original_cfg = evm.ctx().cfg.clone();
    let original_tx = evm.ctx().tx.clone();

    {
        // Read calls are local probes rather than sendable transactions.
        let cfg = &mut evm.ctx_mut().cfg;
        cfg.disable_nonce_check = true;
        cfg.disable_balance_check = true;
        cfg.disable_eip3607 = true;
        cfg.disable_base_fee = true;
        cfg.disable_fee_charge = true;
    }

    let output = operation(evm);

    evm.ctx_mut().cfg = original_cfg;
    evm.ctx_mut().tx = original_tx;

    output
}

pub(super) fn execute_read_call<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &CompleteTransaction,
    chain_id: u64,
    target: Address,
    data: Bytes,
) -> Result<ReadCallOutcome, EVMError<DB::Error>>
where
    DB: Database,
{
    let tx = build_read_call_tx(transaction, chain_id, target, data);
    let result = evm.transact(tx)?.result;

    Ok(match result {
        ExecutionResult::Success { output, .. } => ReadCallOutcome::Success(output.into_data()),
        ExecutionResult::Revert { output, .. } => ReadCallOutcome::Revert(output),
        ExecutionResult::Halt { reason, .. } => ReadCallOutcome::Halt(reason),
    })
}

fn build_read_call_tx(
    transaction: &CompleteTransaction,
    chain_id: u64,
    target: Address,
    data: Bytes,
) -> TxEnv {
    // A distinct preview nonce avoids colliding with the user transaction when
    // both execute against the same local state.
    TxEnv {
        tx_type: TransactionType::Legacy as u8,
        caller: transaction.from,
        gas_limit: READ_CALL_GAS_LIMIT,
        gas_price: 0,
        kind: TxKind::Call(target),
        value: U256::ZERO,
        data,
        nonce: transaction.nonce.saturating_add(1),
        chain_id: Some(chain_id),
        access_list: RevmAccessList::default(),
        gas_priority_fee: None,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
    }
}
