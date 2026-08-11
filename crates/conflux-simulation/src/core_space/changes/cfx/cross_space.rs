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
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
    primitive::{address_from_cfx, u256_from_cfx},
};

sol! {
    function withdrawFromMapped(uint256 value);
}

pub(super) fn collect_cross_space_call(
    trace: &CommittedExecutionTrace,
    frame_position: usize,
    frame_id: FrameId,
    machine: &Machine,
    spec: &Spec,
) -> Result<Option<(CrossSpaceTransferOperation, Vec<usize>)>, ConfluxSimulationError> {
    let frame = trace.frame(frame_id);
    let FrameAction::Call {
        call_type,
        caller,
        target,
        code_address,
        transferred_value,
        calldata_len,
        calldata_prefix,
    } = &frame.action
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

    let Some(selector) = call_selector(*calldata_len, calldata_prefix) else {
        return Ok(None);
    };
    let transfers_to_espace = is_call_create_sig(&selector);
    let withdraws_to_core_space = is_withdraw_sig(&selector);
    if !transfers_to_espace && !withdraws_to_core_space {
        return Ok(None);
    }
    if frame.space != Space::Native
        || *call_type != CallType::Call
        || *target != cross_space_contract
        || *code_address != cross_space_contract
    {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space call did not use the canonical active internal-contract form",
        ));
    }

    let context = CrossSpaceCallContext {
        trace,
        frame_id,
        frame_position,
        caller: *caller,
        transferred_value: *transferred_value,
        calldata_len: *calldata_len,
        calldata_prefix,
    };
    if transfers_to_espace {
        collect_transfer_to_espace(context, cross_space_contract)
    } else {
        collect_withdrawal_to_core_space(context)
    }
}

struct CrossSpaceCallContext<'a> {
    trace: &'a CommittedExecutionTrace,
    frame_id: FrameId,
    frame_position: usize,
    caller: cfx_types::Address,
    transferred_value: cfx_types::U256,
    calldata_len: usize,
    calldata_prefix: &'a [u8],
}

fn collect_transfer_to_espace(
    context: CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
) -> Result<Option<(CrossSpaceTransferOperation, Vec<usize>)>, ConfluxSimulationError> {
    let amount = u256_from_cfx(context.transferred_value);
    if amount.is_zero() {
        return Ok(None);
    }

    let expected_mapped_account = context.caller.evm_map();
    let transfer = unique_matching_transfer(&context, "transfer into eSpace", |transfer| {
        matches!(
            (transfer.from, transfer.to),
            (AddressPocket::Balance(from), AddressPocket::Balance(to))
                if transfer.space == Space::Native
                    && *from == cross_space_contract.with_native_space()
                    && *to == expected_mapped_account
                    && transfer.value == context.transferred_value
        )
    })?;

    Ok(Some((
        CrossSpaceTransferOperation {
            position: Position::new(context.frame_position, 0),
            from: CrossSpaceAddress::CoreSpace(address_from_cfx(context.caller)),
            to: CrossSpaceAddress::Espace(address_from_cfx(expected_mapped_account.address)),
            amount,
        },
        vec![transfer.position],
    )))
}

fn collect_withdrawal_to_core_space(
    context: CrossSpaceCallContext<'_>,
) -> Result<Option<(CrossSpaceTransferOperation, Vec<usize>)>, ConfluxSimulationError> {
    if !context.transferred_value.is_zero() {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space withdrawal transferred call value",
        ));
    }
    if context.calldata_prefix.len() != context.calldata_len {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core cross-space withdrawal calldata was not fully captured",
        ));
    }
    let withdrawal =
        withdrawFromMappedCall::abi_decode_validate(context.calldata_prefix).map_err(|error| {
            ConfluxSimulationError::analysis_failed(format!(
                "Core cross-space withdrawal call is not valid ABI data: {error}"
            ))
        })?;
    let amount = withdrawal.value;
    if amount.is_zero() {
        return Ok(None);
    }

    let mapped_account = context.caller.evm_map();
    let transfer = unique_matching_transfer(&context, "withdrawal into Core Space", |transfer| {
        matches!(
            (transfer.from, transfer.to),
            (AddressPocket::Balance(from), AddressPocket::Balance(to))
                if transfer.space == Space::Native
                    && *from == mapped_account
                    && *to == context.caller.with_native_space()
                    && u256_from_cfx(transfer.value) == amount
        )
    })?;

    Ok(Some((
        CrossSpaceTransferOperation {
            position: Position::new(context.frame_position, 0),
            from: CrossSpaceAddress::Espace(address_from_cfx(mapped_account.address)),
            to: CrossSpaceAddress::CoreSpace(address_from_cfx(context.caller)),
            amount,
        },
        vec![transfer.position],
    )))
}

#[derive(Clone, Copy)]
struct ScopedInternalTransfer<'a> {
    position: usize,
    space: Space,
    from: &'a AddressPocket,
    to: &'a AddressPocket,
    value: cfx_types::U256,
}

fn unique_matching_transfer<'a>(
    context: &'a CrossSpaceCallContext<'_>,
    transfer_name: &str,
    matches_transfer: impl Fn(ScopedInternalTransfer<'_>) -> bool,
) -> Result<ScopedInternalTransfer<'a>, ConfluxSimulationError> {
    let mut matches = context
        .trace
        .internal_transfers_in_scope(Some(context.frame_id))
        .filter_map(scoped_internal_transfer)
        .filter(|transfer| matches_transfer(*transfer));
    let transfer = matches.next().ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(format!(
            "Core cross-space {transfer_name} is missing its committed internal transfer"
        ))
    })?;
    if matches.next().is_some() {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core cross-space {transfer_name} has ambiguous committed internal transfers"
        )));
    }
    Ok(transfer)
}

fn scoped_internal_transfer(event: &TraceEvent) -> Option<ScopedInternalTransfer<'_>> {
    let TraceEvent::InternalTransfer {
        position,
        space,
        from,
        to,
        value,
        ..
    } = event
    else {
        return None;
    };
    Some(ScopedInternalTransfer {
        position: *position,
        space: *space,
        from,
        to,
        value: *value,
    })
}

fn call_selector(calldata_len: usize, calldata_prefix: &[u8]) -> Option<[u8; 4]> {
    if calldata_len < 4 || calldata_prefix.len() < 4 {
        return None;
    }
    let mut selector = [0_u8; 4];
    selector.copy_from_slice(&calldata_prefix[..4]);
    Some(selector)
}
