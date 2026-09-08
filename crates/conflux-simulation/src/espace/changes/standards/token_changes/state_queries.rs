use alloy::primitives::{Address, Bytes, U256};
use contract_standards::getter_abi::{erc20, erc721, erc1155};

use crate::espace::{EspaceChangesError, EspaceReadCallOutcome, EspaceStateReader};

use super::error::token_change_error;

pub(super) fn read_erc20_balance(
    view: &EspaceStateReader,
    contract: Address,
    account: Address,
) -> Result<U256, EspaceChangesError> {
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
    view: &EspaceStateReader,
    contract: Address,
) -> Result<U256, EspaceChangesError> {
    let output = read_call_output(view, contract, erc20::total_supply_call(), "totalSupply()")?;
    erc20::decode_total_supply_output(&output)
        .map_err(|error| token_change_error(format!("invalid totalSupply return data: {error}")))
}

pub(super) fn read_erc1155_balance(
    view: &EspaceStateReader,
    contract: Address,
    account: Address,
    token_id: U256,
) -> Result<U256, EspaceChangesError> {
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
    view: &EspaceStateReader,
    contract: Address,
    owner: Address,
    spender: Address,
) -> Result<U256, EspaceChangesError> {
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
    view: &EspaceStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EspaceChangesError> {
    match view.read_call(contract, erc721::owner_of_call(token_id))? {
        EspaceReadCallOutcome::Success(output) => erc721::decode_owner_of_output(&output)
            .map(Some)
            .map_err(|error| token_change_error(format!("invalid ownerOf return data: {error}"))),
        EspaceReadCallOutcome::Reverted(_) => Ok(None),
        EspaceReadCallOutcome::Failed => Err(token_change_error("ownerOf failed")),
    }
}

pub(super) fn read_erc721_approval(
    view: &EspaceStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EspaceChangesError> {
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
    view: &EspaceStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EspaceChangesError> {
    match view.read_call(contract, erc721::get_approved_call(token_id))? {
        EspaceReadCallOutcome::Success(output) => {
            let address = erc721::decode_get_approved_output(&output).map_err(|error| {
                token_change_error(format!("invalid getApproved return data: {error}"))
            })?;
            Ok(nonzero_address(address))
        }
        EspaceReadCallOutcome::Reverted(_) => Ok(None),
        EspaceReadCallOutcome::Failed => Err(token_change_error("getApproved failed")),
    }
}

pub(super) fn read_operator_approval(
    view: &EspaceStateReader,
    contract: Address,
    owner: Address,
    operator: Address,
) -> Result<bool, EspaceChangesError> {
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
    view: &EspaceStateReader,
    target: Address,
    calldata: Bytes,
    operation: &'static str,
) -> Result<Bytes, EspaceChangesError> {
    match view.read_call(target, calldata)? {
        EspaceReadCallOutcome::Success(output) => Ok(output),
        EspaceReadCallOutcome::Reverted(_) => {
            Err(token_change_error(format!("{operation} reverted")))
        }
        EspaceReadCallOutcome::Failed => Err(token_change_error(format!("{operation} failed"))),
    }
}

pub(super) fn nonzero_address(address: Address) -> Option<Address> {
    (address != Address::ZERO).then_some(address)
}
