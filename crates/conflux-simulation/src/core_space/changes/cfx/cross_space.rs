use alloy_sol_types::{SolCall, sol};
use cfx_executor::{
    executive_observer::AddressPocket,
    internal_contract::{is_call_create_sig, is_withdraw_sig},
    machine::Machine,
};
use cfx_types::{AddressSpaceUtil, Space, address_util::AddressUtil};
use cfx_vm_types::{CallType, Spec};
use contract_standards::legacy::Position;

use super::CrossSpaceTransferOperation;
use crate::{
    ConfluxSimulationError,
    core_space::changes::CrossSpaceAddress,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

sol! {
    function withdrawFromMapped(uint256 value);
}

pub(super) fn collect_cross_space_call(
    observations: &[Observation],
    observation_index: usize,
    machine: &Machine,
    spec: &Spec,
) -> Result<Option<(CrossSpaceTransferOperation, usize)>, ConfluxSimulationError> {
    let Some(Observation::Call {
        position,
        space,
        call_type,
        caller,
        target,
        code_address,
        transferred_value,
        input_len,
        input_prefix,
    }) = observations.get(observation_index)
    else {
        return Ok(None);
    };
    let cross_space_contract =
        cfx_parameters::internal_contract_addresses::CROSS_SPACE_CONTRACT_ADDRESS;

    if *target != cross_space_contract && *code_address != cross_space_contract {
        return Ok(None);
    }
    if machine
        .internal_contracts()
        .contract(&cross_space_contract.with_native_space(), spec)
        .is_none()
    {
        return Ok(None);
    }

    let Some(selector) = call_selector(*input_len, input_prefix) else {
        return Ok(None);
    };
    let transfers_to_espace = is_call_create_sig(&selector);
    let withdraws_to_core_space = is_withdraw_sig(&selector);
    if !transfers_to_espace && !withdraws_to_core_space {
        return Ok(None);
    }
    if *space != Space::Native
        || *call_type != CallType::Call
        || *target != cross_space_contract
        || *code_address != cross_space_contract
    {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space call did not use the canonical active internal-contract form",
        ));
    }
    let call_context = CrossSpaceCallContext {
        observations,
        observation_index,
        position: *position,
        caller: *caller,
        transferred_value: *transferred_value,
        input_len: *input_len,
        input_prefix,
    };

    if transfers_to_espace {
        return collect_transfer_to_espace(call_context, cross_space_contract);
    }

    collect_withdrawal_to_core_space(call_context)
}

struct CrossSpaceCallContext<'a> {
    observations: &'a [Observation],
    observation_index: usize,
    position: usize,
    caller: cfx_types::Address,
    transferred_value: cfx_types::U256,
    input_len: usize,
    input_prefix: &'a [u8],
}

fn collect_transfer_to_espace(
    call_context: CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
) -> Result<Option<(CrossSpaceTransferOperation, usize)>, ConfluxSimulationError> {
    let amount = u256_from_cfx(call_context.transferred_value);
    if amount.is_zero() {
        return Ok(None);
    }

    let transfer = required_transfer(
        call_context.observations,
        call_context.observation_index,
        "transfer into eSpace",
    )?;
    verify_next_position(
        call_context.position,
        transfer.position,
        "transfer into eSpace",
    )?;
    let expected_mapped_account = call_context.caller.evm_map();
    match (transfer.from, transfer.to) {
        (AddressPocket::Balance(from), AddressPocket::Balance(to))
            if transfer.space == Space::Native
                && *from == cross_space_contract.with_native_space()
                && *to == expected_mapped_account
                && transfer.value == call_context.transferred_value =>
        {
            Ok(Some((
                CrossSpaceTransferOperation {
                    position: Position::new(call_context.position, 0),
                    from: CrossSpaceAddress::CoreSpace(address_from_cfx(call_context.caller)),
                    to: CrossSpaceAddress::Espace(address_from_cfx(to.address)),
                    amount,
                },
                2,
            )))
        }
        _ => Err(ConfluxSimulationError::analysis_failed(
            "Core transfer into eSpace did not match its canonical mapped-account movement",
        )),
    }
}

fn collect_withdrawal_to_core_space(
    call_context: CrossSpaceCallContext<'_>,
) -> Result<Option<(CrossSpaceTransferOperation, usize)>, ConfluxSimulationError> {
    if !call_context.transferred_value.is_zero() {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space withdrawal transferred call value",
        ));
    }
    if call_context.input_prefix.len() != call_context.input_len {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space withdrawal input was not fully captured",
        ));
    }
    let withdrawal = withdrawFromMappedCall::abi_decode_validate(call_context.input_prefix)
        .map_err(|error| {
            ConfluxSimulationError::analysis_failed(format!(
                "Core cross-space withdrawal call is not valid ABI data: {error}"
            ))
        })?;
    let amount = withdrawal.value;
    if amount.is_zero() {
        return Ok(None);
    }

    let transfer = required_transfer(
        call_context.observations,
        call_context.observation_index,
        "withdrawal into Core Space",
    )?;
    verify_next_position(
        call_context.position,
        transfer.position,
        "withdrawal into Core Space",
    )?;
    let mapped_account = call_context.caller.evm_map();
    match (transfer.from, transfer.to) {
        (AddressPocket::Balance(from), AddressPocket::Balance(to))
            if transfer.space == Space::Native
                && *from == mapped_account
                && *to == call_context.caller.with_native_space()
                && u256_from_cfx(transfer.value) == amount =>
        {
            Ok(Some((
                CrossSpaceTransferOperation {
                    position: Position::new(call_context.position, 0),
                    from: CrossSpaceAddress::Espace(address_from_cfx(from.address)),
                    to: CrossSpaceAddress::CoreSpace(address_from_cfx(call_context.caller)),
                    amount,
                },
                2,
            )))
        }
        _ => Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space withdrawal did not match its canonical mapped-account movement",
        )),
    }
}

struct ObservedInternalTransfer<'a> {
    position: usize,
    space: Space,
    from: &'a AddressPocket,
    to: &'a AddressPocket,
    value: cfx_types::U256,
}

fn required_transfer<'a>(
    observations: &'a [Observation],
    observation_index: usize,
    operation: &str,
) -> Result<ObservedInternalTransfer<'a>, ConfluxSimulationError> {
    let transfer_index = observation_index.checked_add(1).ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(format!(
            "Core cross-space {operation} observation index overflowed"
        ))
    })?;
    let Some(Observation::InternalTransfer {
        position,
        space,
        from,
        to,
        value,
    }) = observations.get(transfer_index)
    else {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core cross-space {operation} is missing its internal movement"
        )));
    };
    Ok(ObservedInternalTransfer {
        position: *position,
        space: *space,
        from,
        to,
        value: *value,
    })
}

fn verify_next_position(
    previous: usize,
    next: usize,
    operation: &str,
) -> Result<(), ConfluxSimulationError> {
    let expected = previous.checked_add(1).ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(format!(
            "Core cross-space {operation} observation position overflowed"
        ))
    })?;
    if next != expected {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core cross-space {operation} internal movement was not contiguous"
        )));
    }
    Ok(())
}

fn call_selector(input_len: usize, input_prefix: &[u8]) -> Option<[u8; 4]> {
    if input_len < 4 || input_prefix.len() < 4 {
        return None;
    }
    let mut selector = [0_u8; 4];
    selector.copy_from_slice(&input_prefix[..4]);
    Some(selector)
}
