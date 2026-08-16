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

#[cfg(test)]
mod tests {
    use alloy::{
        consensus::{Header, Sealed},
        network::Ethereum,
        primitives::{Address, B256, Bytes, U256},
        providers::{DynProvider, Provider, RootProvider},
        rpc::client::RpcClient,
        transports::mock::Asserter,
    };
    use revm::{bytecode::Bytecode, state::AccountInfo};

    use super::{ReadCallOutcome, execute_read_call, with_read_call_context};
    use crate::{
        CompleteTransaction, CompleteTransactionVariant, EthereumChainSpec, EvmExecutionObserver,
        EvmTransactionExecution, EvmTransactionExecutor, create_database,
    };

    #[test]
    fn metadata_probes_read_post_state_without_committing_their_writes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");
        let caller = Address::repeat_byte(1);
        let contract = Address::repeat_byte(2);
        let beneficiary = Address::repeat_byte(3);
        let block_hash = B256::repeat_byte(4);
        let header = Header {
            beneficiary,
            number: 19_500_000,
            gas_limit: 30_000_000,
            timestamp: 1_720_000_000,
            mix_hash: B256::repeat_byte(5),
            base_fee_per_gas: Some(1),
            excess_blob_gas: Some(0),
            ..Default::default()
        };
        let block = Sealed::new_unchecked(header, block_hash);
        let mut database = create_database(mock_provider(), runtime.handle().clone(), block_hash);
        database.insert_account_info(
            caller,
            AccountInfo::default().with_balance(U256::from(1_000_000)),
        );
        database.insert_account_info(
            contract,
            AccountInfo::default()
                .with_nonce(1)
                .with_code(Bytecode::new_legacy(Bytes::from_static(&[
                    0x60, 0x00, 0x54, 0x60, 0x01, 0x01, 0x80, 0x60, 0x00, 0x55, 0x60, 0x00, 0x52,
                    0x60, 0x20, 0x60, 0x00, 0xf3,
                ]))),
        );
        database
            .insert_account_storage(contract, U256::ZERO, U256::ZERO)
            .expect("cached test account should accept storage");
        database.insert_account_info(beneficiary, AccountInfo::default());

        let transaction = CompleteTransaction {
            from: caller,
            to: Some(contract),
            nonce: 0,
            gas_limit: 100_000,
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 1,
            variant: CompleteTransactionVariant::Legacy { gas_price: 2 },
        };
        let executor = EvmTransactionExecutor::new(
            database,
            block,
            &EthereumChainSpec::mainnet(),
            EvmExecutionObserver::new(),
        )
        .expect("test block should produce a valid execution environment");
        let EvmTransactionExecution::Executed(mut output) = executor
            .execute(&transaction)
            .expect("fixture execution should succeed")
        else {
            panic!("fixture transaction should execute");
        };
        assert!(output.is_success());
        output
            .apply_transition()
            .expect("transaction transition should apply once");

        let outcomes = with_read_call_context(output.evm_mut(), |evm| {
            [
                execute_read_call(evm, &transaction, 1, contract, Bytes::new()),
                execute_read_call(evm, &transaction, 1, contract, Bytes::new()),
            ]
        });
        for outcome in outcomes {
            let ReadCallOutcome::Success(value) =
                outcome.expect("metadata probe should execute successfully")
            else {
                panic!("metadata probe should return successfully");
            };
            assert_eq!(U256::from_be_slice(&value), U256::from(2));
        }
    }

    fn mock_provider() -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(Asserter::new())).erased()
    }
}
