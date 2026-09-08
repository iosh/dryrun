use alloy::primitives::{Address, Bytes, U256};
use contract_standards::getter_abi::{erc20, erc721, erc1155};

use crate::{
    EvmChangeDerivationError,
    state::{EvmReadCallOutcome, EvmStateReader},
};

use super::{error::token_change_error, events::nonzero_address};

pub(super) fn read_erc20_balance(
    view: &EvmStateReader,
    contract: Address,
    account: Address,
) -> Result<U256, EvmChangeDerivationError> {
    let output = read_call_output(
        view,
        contract,
        erc20::balance_of_call(account),
        "balanceOf(address)",
    )?;
    erc20::decode_balance_of_output(&output)
        .map_err(|error| token_change_error(format!("invalid balanceOf return data: {error}")))
}

pub(super) fn read_erc20_total_supply(
    view: &EvmStateReader,
    contract: Address,
) -> Result<U256, EvmChangeDerivationError> {
    let output = read_call_output(view, contract, erc20::total_supply_call(), "totalSupply()")?;
    erc20::decode_total_supply_output(&output)
        .map_err(|error| token_change_error(format!("invalid totalSupply return data: {error}")))
}

pub(super) fn read_erc1155_balance(
    view: &EvmStateReader,
    contract: Address,
    account: Address,
    token_id: U256,
) -> Result<U256, EvmChangeDerivationError> {
    let output = read_call_output(
        view,
        contract,
        erc1155::balance_of_call(account, token_id),
        "balanceOf(address,uint256)",
    )?;
    erc1155::decode_balance_of_output(&output).map_err(|error| {
        token_change_error(format!("invalid ERC-1155 balanceOf return data: {error}"))
    })
}

pub(super) fn read_allowance(
    view: &EvmStateReader,
    contract: Address,
    owner: Address,
    spender: Address,
) -> Result<U256, EvmChangeDerivationError> {
    let output = read_call_output(
        view,
        contract,
        erc20::allowance_call(owner, spender),
        "allowance(address,address)",
    )?;
    erc20::decode_allowance_output(&output)
        .map_err(|error| token_change_error(format!("invalid allowance return data: {error}")))
}

pub(super) fn read_erc721_owner(
    view: &EvmStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EvmChangeDerivationError> {
    match view.read_call(contract, erc721::owner_of_call(token_id))? {
        EvmReadCallOutcome::Success(output) => erc721::decode_owner_of_output(&output)
            .map(Some)
            .map_err(|error| token_change_error(format!("invalid ownerOf return data: {error}"))),
        EvmReadCallOutcome::Reverted(_) => Ok(None),
        EvmReadCallOutcome::Halted { reason } => {
            Err(token_change_error(format!("ownerOf halted: {reason}")))
        }
    }
}

pub(super) fn read_erc721_approval(
    view: &EvmStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EvmChangeDerivationError> {
    let output = read_call_output(
        view,
        contract,
        erc721::get_approved_call(token_id),
        "getApproved(uint256)",
    )?;
    let address = erc721::decode_get_approved_output(&output)
        .map_err(|error| token_change_error(format!("invalid getApproved return data: {error}")))?;
    Ok(nonzero_address(address))
}

pub(super) fn read_erc721_approval_optional(
    view: &EvmStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EvmChangeDerivationError> {
    match view.read_call(contract, erc721::get_approved_call(token_id))? {
        EvmReadCallOutcome::Success(output) => {
            let address = erc721::decode_get_approved_output(&output).map_err(|error| {
                token_change_error(format!("invalid getApproved return data: {error}"))
            })?;
            Ok(nonzero_address(address))
        }
        EvmReadCallOutcome::Reverted(_) => Ok(None),
        EvmReadCallOutcome::Halted { reason } => {
            Err(token_change_error(format!("getApproved halted: {reason}")))
        }
    }
}

pub(super) fn read_operator_approval(
    view: &EvmStateReader,
    contract: Address,
    owner: Address,
    operator: Address,
) -> Result<bool, EvmChangeDerivationError> {
    let output = read_call_output(
        view,
        contract,
        erc721::is_approved_for_all_call(owner, operator),
        "isApprovedForAll(address,address)",
    )?;
    erc721::decode_is_approved_for_all_output(&output).map_err(|error| {
        token_change_error(format!("invalid isApprovedForAll return data: {error}"))
    })
}

pub(super) fn read_call_output(
    view: &EvmStateReader,
    target: Address,
    calldata: Bytes,
    operation: &'static str,
) -> Result<Bytes, EvmChangeDerivationError> {
    match view.read_call(target, calldata)? {
        EvmReadCallOutcome::Success(output) => Ok(output),
        EvmReadCallOutcome::Reverted(_) => Err(token_change_error(format!("{operation} reverted"))),
        EvmReadCallOutcome::Halted { reason } => {
            Err(token_change_error(format!("{operation} halted: {reason}")))
        }
    }
}
