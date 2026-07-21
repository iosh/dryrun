use alloy_primitives::{Address, U256};
use revm::{
    Context, InspectEvm, MainBuilder, MainContext,
    context::TxEnv,
    database::InMemoryDB,
    primitives::TxKind,
    state::{Account, AccountInfo, EvmState},
};

use super::{
    super::{
        candidate::collect_candidates, error::TransactionChangesError,
        native::check_native_balances, observation::ChangeObservationInspector,
    },
    support::native_candidate,
};

fn state_account(original_balance: U256, current_balance: U256) -> Account {
    let mut account = Account::from(AccountInfo::default().with_balance(original_balance));
    account.info.balance = current_balance;
    account.mark_touch();
    account
}

fn native_state<const N: usize>(accounts: [(Address, U256, U256); N]) -> EvmState {
    accounts
        .into_iter()
        .map(|(address, original_balance, current_balance)| {
            (address, state_account(original_balance, current_balance))
        })
        .collect()
}

#[test]
fn reconciles_revm_transfer_and_fee_only_state() {
    const GAS_LIMIT: u64 = 21_000;
    const GAS_PRICE: u128 = 10;
    const BASE_FEE: u64 = 3;

    let caller = Address::repeat_byte(0x01);
    let receiver = Address::repeat_byte(0x02);
    let beneficiary = Address::repeat_byte(0x03);
    let mut db = InMemoryDB::default();
    db.insert_account_info(
        caller,
        AccountInfo::default().with_balance(U256::from(1_000_000_u64)),
    );
    db.insert_account_info(
        receiver,
        AccountInfo::default().with_balance(U256::from(10_u64)),
    );
    db.insert_account_info(
        beneficiary,
        AccountInfo::default().with_balance(U256::from(5_u64)),
    );

    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_block_chained(|block| {
            block.basefee = BASE_FEE;
            block.beneficiary = beneficiary;
        })
        .build_mainnet_with_inspector(ChangeObservationInspector::new());
    let result_and_state = evm
        .inspect_tx(
            TxEnv::builder()
                .caller(caller)
                .kind(TxKind::Call(receiver))
                .value(U256::from(200_u64))
                .gas_limit(GAS_LIMIT)
                .gas_price(GAS_PRICE)
                .build()
                .expect("valid native transfer transaction"),
        )
        .expect("native transfer execution");
    let observations = std::mem::take(&mut evm.inspector).into_observations();
    let candidates = collect_candidates(&observations).expect("native transfer candidate");
    let gas = result_and_state.result.gas();
    let gas_precharge = U256::from(gas.limit()) * U256::from(GAS_PRICE);
    let fee = U256::from(gas.used()) * U256::from(GAS_PRICE);
    let caller_refund = gas_precharge - fee;
    let beneficiary_reward = U256::from(gas.used()) * U256::from(GAS_PRICE - u128::from(BASE_FEE));

    check_native_balances(
        &result_and_state.state,
        &candidates,
        caller,
        beneficiary,
        gas_precharge,
        caller_refund,
        beneficiary_reward,
    )
    .expect("Revm native balances should reconcile");

    let fee_only_state = native_state([
        (caller, U256::from(1_000_u64), U256::from(940_u64)),
        (beneficiary, U256::from(5_u64), U256::from(55_u64)),
    ]);

    check_native_balances(
        &fee_only_state,
        &[],
        caller,
        beneficiary,
        U256::from(100_u64),
        U256::from(40_u64),
        U256::from(50_u64),
    )
    .expect("fee-only balances should reconcile");
}

#[test]
fn rejects_balance_received_after_selfdestruct() {
    let caller = Address::repeat_byte(0x01);
    let destroyed = Address::repeat_byte(0x02);
    let target = Address::repeat_byte(0x03);
    let mut state = native_state([
        (caller, U256::from(25_u64), U256::ZERO),
        (destroyed, U256::from(300_u64), U256::from(25_u64)),
        (target, U256::ZERO, U256::from(300_u64)),
    ]);
    state
        .get_mut(&destroyed)
        .expect("destroyed account")
        .mark_selfdestruct();
    let candidates = [
        native_candidate(0, destroyed, target, U256::from(300_u64)),
        native_candidate(1, caller, destroyed, U256::from(25_u64)),
    ];

    let error = check_native_balances(
        &state,
        &candidates,
        caller,
        target,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("balance deleted at commit must not be reported as a transfer");

    assert!(matches!(
        error,
        TransactionChangesError::NativeBalanceMismatch {
            address,
            replayed_balance,
            state_balance,
        } if address == destroyed
            && replayed_balance == U256::from(25_u64)
            && state_balance == U256::ZERO
    ));
}

#[test]
fn rejects_invalid_balance_replay() {
    let caller = Address::repeat_byte(0x01);
    let target = Address::repeat_byte(0x02);
    let unrelated = Address::repeat_byte(0x03);
    let missing_state = native_state([(caller, U256::from(10_u64), U256::from(9_u64))]);

    let missing_error = check_native_balances(
        &missing_state,
        &[native_candidate(0, caller, target, U256::from(1_u64))],
        caller,
        caller,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("missing target account must fail");
    assert!(matches!(
        missing_error,
        TransactionChangesError::NativeAccountMissing { address } if address == target
    ));

    let underflow_state = native_state([(caller, U256::from(10_u64), U256::from(10_u64))]);
    let underflow_error = check_native_balances(
        &underflow_state,
        &[native_candidate(0, caller, caller, U256::from(20_u64))],
        caller,
        caller,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("underfunded self-transfer must fail");
    assert!(matches!(
        underflow_error,
        TransactionChangesError::NativeBalanceUnderflow { address, .. }
            if address == caller
    ));

    let unexplained_state = native_state([
        (caller, U256::from(10_u64), U256::from(10_u64)),
        (unrelated, U256::from(5_u64), U256::from(6_u64)),
    ]);
    let mismatch_error = check_native_balances(
        &unexplained_state,
        &[],
        caller,
        caller,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("unexplained state balance change must fail");
    assert!(matches!(
        mismatch_error,
        TransactionChangesError::NativeBalanceMismatch {
            address,
            replayed_balance,
            state_balance,
        } if address == unrelated
            && replayed_balance == U256::from(5_u64)
            && state_balance == U256::from(6_u64)
    ));
}
