use std::sync::Arc;

use cfx_types::{Address, H256, U256, address_util::AddressUtil};
use keccak_hash::{KECCAK_EMPTY, keccak};

use thiserror::Error;

use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use primitives::{
    CodeInfo, DepositInfo, DepositList, VoteStakeInfo, VoteStakeList,
    account::{BasicAccount, ContractAccount, EthereumAccount, SponsorInfo, StoragePoints},
    storage::StorageValue,
};

use crate::state::rpc_types::NativeSponsorInfo;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum StateValueEncodingError {
    #[error("code hash mismatch: expected {expected:?}, got {actual:?}")]
    CodeHashMismatch { expected: H256, actual: H256 },
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

pub(crate) fn encode_native_contract_account(
    balance: U256,
    nonce: U256,
    code_hash: H256,
    staking_balance: U256,
    collateral_for_storage: U256,
    accumulated_interest_return: U256,
    admin: Address,
    sponsor_info: NativeSponsorInfo,
) -> Option<Box<[u8]>> {
    let sponsor_info = native_sponsor_info_from_rpc(sponsor_info);

    if balance.is_zero()
        && nonce.is_zero()
        && code_hash == KECCAK_EMPTY
        && staking_balance.is_zero()
        && collateral_for_storage.is_zero()
        && accumulated_interest_return.is_zero()
        && admin.is_zero()
        && sponsor_info == SponsorInfo::default()
    {
        return None;
    }

    Some(
        rlp::encode(&ContractAccount {
            balance,
            nonce,
            code_hash,
            staking_balance,
            collateral_for_storage,
            accumulated_interest_return,
            admin,
            sponsor_info,
        })
        .to_vec()
        .into_boxed_slice(),
    )
}

pub(crate) fn should_encode_native_contract_account(address: Address, code_hash: H256) -> bool {
    (code_hash != KECCAK_EMPTY && !code_hash.is_zero()) || address.is_contract_address()
}

pub(crate) fn encode_native_code(
    expected_code_hash: H256,
    owner: Address,
    code: Vec<u8>,
) -> Result<Box<[u8]>, StateValueEncodingError> {
    let actual_code_hash = keccak(&code);
    if actual_code_hash != expected_code_hash {
        return Err(StateValueEncodingError::CodeHashMismatch {
            expected: expected_code_hash,
            actual: actual_code_hash,
        });
    }

    Ok(rlp::encode(&CodeInfo {
        code: Arc::new(code),
        owner,
    })
    .to_vec()
    .into_boxed_slice())
}

pub(crate) fn encode_native_deposit_list(deposits: Vec<DepositInfo>) -> Option<Box<[u8]>> {
    if deposits.is_empty() {
        return None;
    }

    Some(
        rlp::encode(&DepositList(deposits))
            .to_vec()
            .into_boxed_slice(),
    )
}

pub(crate) fn encode_native_vote_list(votes: Vec<VoteStakeInfo>) -> Option<Box<[u8]>> {
    if votes.is_empty() {
        return None;
    }

    Some(
        rlp::encode(&VoteStakeList(votes))
            .to_vec()
            .into_boxed_slice(),
    )
}

pub(crate) fn encode_native_storage_slot(value: U256) -> Box<[u8]> {
    rlp::encode(&StorageValue { value, owner: None })
        .to_vec()
        .into_boxed_slice()
}

fn native_sponsor_info_from_rpc(info: NativeSponsorInfo) -> SponsorInfo {
    let unused = info.available_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT;
    let used = info.used_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT;

    SponsorInfo {
        sponsor_for_gas: info.sponsor_for_gas.into(),
        sponsor_for_collateral: info.sponsor_for_collateral.into(),
        sponsor_gas_bound: info.sponsor_gas_bound,
        sponsor_balance_for_gas: info.sponsor_balance_for_gas,
        sponsor_balance_for_collateral: info.sponsor_balance_for_collateral,
        storage_points: (!unused.is_zero() || !used.is_zero())
            .then_some(StoragePoints { unused, used }),
    }
}

// eSpace storage slots are encoded as StorageValue with no owner.
pub(crate) fn encode_espace_storage_slot(value: U256) -> Box<[u8]> {
    rlp::encode(&StorageValue { value, owner: None })
        .to_vec()
        .into_boxed_slice()
}

// eSpace code values follow the upstream CodeInfo RLP layout. CodeKey already
// carries the expected hash, so verify the code bytes before encoding.
pub(crate) fn encode_espace_code(
    expected_code_hash: H256,
    code: Arc<Vec<u8>>,
) -> Result<Box<[u8]>, StateValueEncodingError> {
    let actual_code_hash = keccak(code.as_ref());
    if actual_code_hash != expected_code_hash {
        return Err(StateValueEncodingError::CodeHashMismatch {
            expected: expected_code_hash,
            actual: actual_code_hash,
        });
    }

    Ok(rlp::encode(&CodeInfo {
        code,
        owner: Address::zero(),
    })
    .to_vec()
    .into_boxed_slice())
}

pub(crate) fn encode_espace_account(balance: U256, nonce: U256, code: &[u8]) -> Option<Box<[u8]>> {
    if balance.is_zero() && nonce.is_zero() && code.is_empty() {
        return None;
    }

    Some(
        rlp::encode(&EthereumAccount {
            balance,
            nonce,
            code_hash: keccak(code),
        })
        .to_vec()
        .into_boxed_slice(),
    )
}
