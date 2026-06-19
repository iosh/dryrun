use std::sync::Arc;

use cfx_types::{Address, H256, U256};
use keccak_hash::keccak;
use primitives::{
    CodeInfo,
    account::{BasicAccount, EthereumAccount},
    storage::StorageValue,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum StateValueCodecError {
    #[error("eSpace code hash mismatch: expected {expected:?}, got {actual:?}")]
    EspaceCodeHashMismatch { expected: H256, actual: H256 },
}

pub(crate) fn encode_native_u256(value: U256) -> Box<[u8]> {
    rlp::encode(&value).to_vec().into_boxed_slice()
}

pub(crate) fn encode_native_basic_account(
    balance: U256,
    nonce: U256,
    staking_balance: U256,
    collateral_for_storage: U256,
    accumulated_interest_return: U256,
) -> Option<Box<[u8]>> {
    if balance.is_zero()
        && nonce.is_zero()
        && staking_balance.is_zero()
        && collateral_for_storage.is_zero()
        && accumulated_interest_return.is_zero()
    {
        return None;
    }

    Some(
        rlp::encode(&BasicAccount {
            balance,
            nonce,
            staking_balance,
            collateral_for_storage,
            accumulated_interest_return,
        })
        .to_vec()
        .into_boxed_slice(),
    )
}

// eSpace storage slots are encoded as StorageValue with no owner.
pub(crate) fn encode_espace_storage_slot(value: U256) -> Box<[u8]> {
    rlp::encode(&StorageValue { value, owner: None })
        .to_vec()
        .into_boxed_slice()
}

// eSpace code values follow the upstream CodeInfo RLP layout.
// CodeKey already carries the expected hash, so we verify the code bytes first.

pub(crate) fn encode_espace_code(
    expected_code_hash: H256,
    code: Vec<u8>,
) -> Result<Box<[u8]>, StateValueCodecError> {
    let actual_code_hash = keccak(&code);
    if actual_code_hash != expected_code_hash {
        return Err(StateValueCodecError::EspaceCodeHashMismatch {
            expected: expected_code_hash,
            actual: actual_code_hash,
        });
    }

    Ok(rlp::encode(&CodeInfo {
        code: Arc::new(code),
        owner: Address::zero(),
    })
    .to_vec()
    .into_boxed_slice())
}

pub(crate) fn encode_espace_account(
    balance: U256,
    nonce: U256,
    code: Vec<u8>,
) -> Option<Box<[u8]>> {
    if balance.is_zero() && nonce.is_zero() && code.is_empty() {
        return None;
    }

    Some(
        rlp::encode(&EthereumAccount {
            balance,
            nonce,
            code_hash: keccak(&code),
        })
        .to_vec()
        .into_boxed_slice(),
    )
}
