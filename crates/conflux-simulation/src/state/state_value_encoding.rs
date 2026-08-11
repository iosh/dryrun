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

use crate::state::rpc_types::{CoreSpaceRpcAccount, CoreSpaceSponsorInfo};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum StateValueEncodingError {
    #[error("code hash mismatch: expected {expected:?}, got {actual:?}")]
    CodeHashMismatch { expected: H256, actual: H256 },

    #[error("Core Space account collateral total {total} is less than token collateral {token}")]
    StorageCollateralUnderflow { total: U256, token: U256 },

    #[error("basic Core Space account has storage-point collateral {value}")]
    BasicAccountStoragePointCollateral { value: U256 },

    #[error("available storage-point units {units} overflow when converted to collateral in drips")]
    AvailableStoragePointCollateralOverflow { units: U256 },
}

pub(crate) fn used_storage_point_collateral(
    total_collateral_for_storage: U256,
    token_collateral_for_storage: U256,
) -> Result<U256, StateValueEncodingError> {
    total_collateral_for_storage
        .checked_sub(token_collateral_for_storage)
        .ok_or(StateValueEncodingError::StorageCollateralUnderflow {
            total: total_collateral_for_storage,
            token: token_collateral_for_storage,
        })
}

pub(crate) fn encode_core_space_u256(value: U256) -> Box<[u8]> {
    rlp::encode(&value).to_vec().into_boxed_slice()
}

pub(crate) fn encode_core_space_basic_account(
    balance: U256,
    nonce: U256,
    staking_balance: U256,
    token_collateral_for_storage: U256,
    accumulated_interest_return: U256,
) -> Option<Box<[u8]>> {
    if balance.is_zero()
        && nonce.is_zero()
        && staking_balance.is_zero()
        && token_collateral_for_storage.is_zero()
        && accumulated_interest_return.is_zero()
    {
        return None;
    }

    Some(
        rlp::encode(&BasicAccount {
            balance,
            nonce,
            staking_balance,
            collateral_for_storage: token_collateral_for_storage,
            accumulated_interest_return,
        })
        .to_vec()
        .into_boxed_slice(),
    )
}

pub(crate) fn encode_core_space_contract_account(
    account: &CoreSpaceRpcAccount,
    token_collateral_for_storage: U256,
    used_storage_point_collateral: U256,
    sponsor_info: CoreSpaceSponsorInfo,
) -> Result<Option<Box<[u8]>>, StateValueEncodingError> {
    let sponsor_info =
        core_space_sponsor_info_from_rpc(sponsor_info, used_storage_point_collateral)?;

    if account.balance.is_zero()
        && account.nonce.is_zero()
        && account.code_hash == KECCAK_EMPTY
        && account.staking_balance.is_zero()
        && token_collateral_for_storage.is_zero()
        && account.accumulated_interest_return.is_zero()
        && account.admin.hex_address.is_zero()
        && sponsor_info == SponsorInfo::default()
    {
        return Ok(None);
    }

    Ok(Some(
        rlp::encode(&ContractAccount {
            balance: account.balance,
            nonce: account.nonce,
            code_hash: account.code_hash,
            staking_balance: account.staking_balance,
            collateral_for_storage: token_collateral_for_storage,
            accumulated_interest_return: account.accumulated_interest_return,
            admin: account.admin.hex_address,
            sponsor_info,
        })
        .to_vec()
        .into_boxed_slice(),
    ))
}

pub(crate) fn should_encode_core_space_contract_account(address: Address, code_hash: H256) -> bool {
    (code_hash != KECCAK_EMPTY && !code_hash.is_zero()) || address.is_contract_address()
}

// CodeKey carries the expected hash. Verify RPC-returned bytes before encoding
// the upstream CodeInfo layout; the caller supplies the space-specific owner.
pub(crate) fn encode_code(
    expected_code_hash: H256,
    owner: Address,
    code: Arc<Vec<u8>>,
) -> Result<Box<[u8]>, StateValueEncodingError> {
    let actual_code_hash = keccak(code.as_ref());
    if actual_code_hash != expected_code_hash {
        return Err(StateValueEncodingError::CodeHashMismatch {
            expected: expected_code_hash,
            actual: actual_code_hash,
        });
    }

    Ok(rlp::encode(&CodeInfo { code, owner })
        .to_vec()
        .into_boxed_slice())
}

pub(crate) fn encode_core_space_deposit_list(deposits: Vec<DepositInfo>) -> Option<Box<[u8]>> {
    if deposits.is_empty() {
        return None;
    }

    Some(
        rlp::encode(&DepositList(deposits))
            .to_vec()
            .into_boxed_slice(),
    )
}

pub(crate) fn encode_core_space_vote_list(votes: Vec<VoteStakeInfo>) -> Option<Box<[u8]>> {
    if votes.is_empty() {
        return None;
    }

    Some(
        rlp::encode(&VoteStakeList(votes))
            .to_vec()
            .into_boxed_slice(),
    )
}

// Upstream StorageValue encodes an ownerless slot as the bare U256 value.
pub(crate) fn encode_storage_slot(value: U256) -> Box<[u8]> {
    rlp::encode(&StorageValue { value, owner: None })
        .to_vec()
        .into_boxed_slice()
}

fn core_space_sponsor_info_from_rpc(
    info: CoreSpaceSponsorInfo,
    used_storage_point_collateral: U256,
) -> Result<SponsorInfo, StateValueEncodingError> {
    let unused_storage_point_collateral = info
        .available_storage_point_units
        .checked_mul(*DRIPS_PER_STORAGE_COLLATERAL_UNIT)
        .ok_or(
            StateValueEncodingError::AvailableStoragePointCollateralOverflow {
                units: info.available_storage_point_units,
            },
        )?;

    Ok(SponsorInfo {
        sponsor_for_gas: info.sponsor_for_gas.into(),
        sponsor_for_collateral: info.sponsor_for_collateral.into(),
        sponsor_gas_bound: info.sponsor_gas_bound,
        sponsor_balance_for_gas: info.sponsor_balance_for_gas,
        sponsor_balance_for_collateral: info.sponsor_balance_for_collateral,
        storage_points: Some(StoragePoints {
            unused: unused_storage_point_collateral,
            used: used_storage_point_collateral,
        }),
    })
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
