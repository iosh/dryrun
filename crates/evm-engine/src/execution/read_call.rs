use alloy_primitives::{Address, Bytes, U256};
use revm::{
    Database, ExecuteEvm,
    context::TxEnv,
    context_interface::{
        result::ExecutionResult,
        transaction::{AccessList as RevmAccessList, TransactionType},
    },
    handler::EvmTr,
    primitives::TxKind,
};

use crate::EvmTransaction;

use super::MainnetEvmWithDb;

const READ_CALL_GAS_LIMIT: u64 = 100_000;

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

// Executes a read-like call against the current in-memory state. The returned
// state is intentionally discarded instead of being committed.
pub(super) fn execute_read_call<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    target: Address,
    data: Bytes,
) -> Option<Bytes>
where
    DB: Database,
{
    let tx = build_read_call_tx(transaction, chain_id, target, data);
    let result = evm.transact(tx).ok()?.result;

    match result {
        ExecutionResult::Success { output, .. } => Some(output.into_data()),
        ExecutionResult::Revert { .. } | ExecutionResult::Halt { .. } => None,
    }
}

fn build_read_call_tx(
    transaction: &EvmTransaction,
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

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes, U256};
    use revm::{
        Context, Database, MainBuilder, MainContext,
        context::TxEnv,
        database::InMemoryDB,
        handler::EvmTr,
        primitives::TxKind,
        state::{AccountInfo, Bytecode, bytecode::opcode},
    };

    use crate::{EvmTransaction, EvmTransactionVariant};

    use super::{execute_read_call, with_read_call_context};

    #[test]
    fn restores_read_call_context_without_committing_state() {
        let caller = Address::repeat_byte(0x01);
        let contract = Address::repeat_byte(0x02);

        let code = vec![
            opcode::PUSH1,
            0x2a,
            opcode::PUSH1,
            0x00,
            opcode::SSTORE,
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x00,
            opcode::RETURN,
        ];

        let mut db = InMemoryDB::default();
        db.insert_account_info(
            caller,
            AccountInfo::default().with_balance(U256::from(1_000_000_000_u64)),
        );
        db.insert_account_info(
            contract,
            AccountInfo::default()
                .with_nonce(1)
                .with_code(Bytecode::new_raw(Bytes::from(code))),
        );

        let mut evm = Context::mainnet().with_db(db).build_mainnet();

        let original_tx = TxEnv {
            caller: Address::repeat_byte(0x03),
            kind: TxKind::Call(Address::repeat_byte(0x04)),
            nonce: 7,
            gas_limit: 21_000,
            ..TxEnv::default()
        };
        evm.ctx_mut().tx = original_tx.clone();
        let original_cfg = evm.ctx().cfg.clone();

        let transaction = EvmTransaction {
            chain_id: 1,
            from: caller,
            to: Some(contract),
            nonce: 0,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            variant: EvmTransactionVariant::Legacy { gas_price: 0 },
        };

        let output = with_read_call_context(&mut evm, |evm| {
            assert!(evm.ctx().cfg.disable_nonce_check);
            assert!(evm.ctx().cfg.disable_balance_check);
            assert!(evm.ctx().cfg.disable_eip3607);
            assert!(evm.ctx().cfg.disable_base_fee);
            assert!(evm.ctx().cfg.disable_fee_charge);

            execute_read_call(evm, &transaction, 1, contract, Bytes::new())
        });

        assert_eq!(output, Some(Bytes::new()));
        assert_eq!(evm.ctx().cfg, original_cfg);
        assert_eq!(evm.ctx().tx, original_tx);

        let stored_value = evm
            .ctx_mut()
            .journaled_state
            .database
            .storage(contract, U256::ZERO)
            .expect("contract storage");

        assert_eq!(stored_value, U256::ZERO);
    }
}
