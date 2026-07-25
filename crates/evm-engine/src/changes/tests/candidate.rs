use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use contract_standards::{Position, StandardCandidate};

use super::super::{
    contract_candidates::collect_contract_candidates, error::TransactionChangesError,
    native::collect_native_candidates, observation::Observation,
};

fn indexed_address(address: Address) -> B256 {
    address.into_word()
}

fn event_observation<const N: usize>(
    address: Address,
    signature: &str,
    indexed_topics: [B256; N],
    data: Bytes,
) -> Observation {
    let mut topics = Vec::with_capacity(N + 1);
    topics.push(keccak256(signature));
    topics.extend(indexed_topics);

    Observation::Log {
        address,
        topics,
        data,
    }
}

fn call_observation(caller: Address, target: Address, value: u64) -> Observation {
    Observation::Call {
        caller,
        target,
        value: U256::from(value),
        input_len: 0,
        input_prefix: Bytes::new(),
    }
}

fn weth9_event_observation(
    token: Address,
    signature: &str,
    account: Address,
    amount: u64,
) -> Observation {
    event_observation(
        token,
        signature,
        [indexed_address(account)],
        Bytes::from(U256::from(amount).to_be_bytes_vec()),
    )
}

#[test]
fn matches_weth_calls() {
    let weth = Address::repeat_byte(0x01);
    let account = Address::repeat_byte(0x02);
    let candidates = collect_contract_candidates(&[
        call_observation(account, weth, 5),
        weth9_event_observation(weth, "Deposit(address,uint256)", account, 5),
        call_observation(weth, account, 2),
        weth9_event_observation(weth, "Withdrawal(address,uint256)", account, 2),
        weth9_event_observation(weth, "Deposit(address,uint256)", account, 5),
    ])
    .expect("WETH9 candidates");

    assert_eq!(
        candidates,
        vec![
            StandardCandidate::erc20_movement(
                Position::new(1, 0),
                weth,
                Address::ZERO,
                account,
                U256::from(5_u64),
            ),
            StandardCandidate::erc20_movement(
                Position::new(3, 0),
                weth,
                account,
                Address::ZERO,
                U256::from(2_u64),
            ),
        ]
    );
}

#[test]
fn maps_malformed_event_error() {
    let token = Address::repeat_byte(0x01);
    let malformed = event_observation(token, "Transfer(address,address,uint256)", [], Bytes::new());

    let error = collect_contract_candidates(&[malformed]).expect_err("malformed transfer event");

    assert_eq!(
        error.details(),
        "transaction changes failed: failed to decode event at observation 0: \
         malformed Transfer event: expected ERC-20 or ERC-721 Transfer shape"
    );
}

#[test]
fn rejects_nonzero_selfdestruct_to_self() {
    let contract = Address::repeat_byte(0x01);
    let amount = U256::from(7_u64);
    let error = collect_native_candidates(&[Observation::SelfDestruct {
        contract,
        target: contract,
        amount,
    }])
    .expect_err("selfdestruct-to-self");

    assert!(matches!(
        error,
        TransactionChangesError::UnsupportedSelfDestructToSelf {
            observation_index: 0,
            contract: error_contract,
            amount: error_amount,
        } if error_contract == contract && error_amount == amount
    ));
}
