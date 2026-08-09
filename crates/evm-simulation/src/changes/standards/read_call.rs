use alloy::primitives::{Address, Bytes, U256};
use revm::{
    ExecuteEvm,
    context::TxEnv,
    context_interface::{
        result::{EVMError, ExecutionResult},
        transaction::{AccessList as RevmAccessList, TransactionType},
    },
    handler::EvmTr,
    primitives::TxKind,
};

use crate::{CompleteTransaction, EvmExecutionObserver, execution::MainnetEvm};

const METADATA_CALL_GAS_LIMIT: u64 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReadCallOutcome {
    Success(Bytes),
    Reverted,
    Halted,
}

pub(super) fn with_read_call_context<T>(
    evm: &mut MainnetEvm<EvmExecutionObserver>,
    operation: impl FnOnce(&mut MainnetEvm<EvmExecutionObserver>) -> T,
) -> T {
    let original_cfg = evm.ctx().cfg.clone();
    let original_tx = evm.ctx().tx.clone();

    {
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

pub(super) fn execute_read_call(
    evm: &mut MainnetEvm<EvmExecutionObserver>,
    transaction: &CompleteTransaction,
    chain_id: u64,
    target: Address,
    data: Bytes,
) -> Result<ReadCallOutcome, EVMError<revm::database::AlloyDBError>> {
    let tx = build_read_call_tx(transaction, chain_id, target, data);
    let result = evm.transact(tx);

    // A probe must not leave instrumentation behind even if a future Revm
    // execution path invokes the configured inspector.
    let _ = evm.inspector.take_observations();

    Ok(match result?.result {
        ExecutionResult::Success { output, .. } => ReadCallOutcome::Success(output.into_data()),
        ExecutionResult::Revert { .. } => ReadCallOutcome::Reverted,
        ExecutionResult::Halt { .. } => ReadCallOutcome::Halted,
    })
}

fn build_read_call_tx(
    transaction: &CompleteTransaction,
    chain_id: u64,
    target: Address,
    data: Bytes,
) -> TxEnv {
    TxEnv {
        tx_type: TransactionType::Legacy as u8,
        caller: transaction.from,
        gas_limit: METADATA_CALL_GAS_LIMIT,
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
