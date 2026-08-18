use alloy_primitives::{Address, FixedBytes, U256};
use alloy_sol_types::{SolCall, SolEvent, sol};
use cfx_executor::{executive_observer::AddressPocket, machine::Machine};
use cfx_types::{AddressSpaceUtil, Space, address_util::AddressUtil};
use cfx_vm_types::{CallType, Spec};

use super::CrossSpaceTransferOperation;
use crate::{
    core_space::{CoreSpaceChangesError, changes::ChangePosition},
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx, u256_from_cfx},
};

sol! {
    function createEVM(bytes init);
    function transferEVM(bytes20 receiver);
    function callEVM(bytes20 receiver, bytes data);
    function withdrawFromMapped(uint256 value);

    event Call(
        bytes20 indexed sender,
        bytes20 indexed receiver,
        uint256 value,
        uint256 nonce,
        bytes data
    );
    event Create(
        bytes20 indexed sender,
        bytes20 indexed receiver,
        uint256 value,
        uint256 nonce,
        bytes data
    );
    event Withdraw(
        bytes20 indexed sender,
        address indexed receiver,
        uint256 value,
        uint256 nonce
    );
}

pub(super) fn collect_cross_space_call(
    trace: &CommittedExecutionTrace,
    frame_position: usize,
    frame_id: FrameId,
    machine: &Machine,
    spec: &Spec,
) -> Result<Option<(CrossSpaceTransferOperation, Vec<usize>)>, CoreSpaceChangesError> {
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
    let call_kind = match selector {
        selector if selector == withdrawFromMappedCall::SELECTOR => CrossSpaceCallKind::Withdrawal,
        selector if selector == createEVMCall::SELECTOR => CrossSpaceCallKind::Create,
        selector if selector == transferEVMCall::SELECTOR || selector == callEVMCall::SELECTOR => {
            CrossSpaceCallKind::Call
        }
        _ => return Ok(None),
    };
    if frame.space != Space::Native
        || *call_type != CallType::Call
        || *target != cross_space_contract
        || *code_address != cross_space_contract
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core cross-space call did not use the canonical active internal-contract form",
        ));
    }

    let context = CrossSpaceCallContext {
        trace,
        frame_id,
        frame_position,
        caller: *caller,
        transferred_value: *transferred_value,
    };
    match call_kind {
        CrossSpaceCallKind::Withdrawal => {
            collect_withdrawal_to_core_space(context, cross_space_contract)
        }
        CrossSpaceCallKind::Create => {
            collect_transfer_to_espace(context, EspaceTransferKind::Create, cross_space_contract)
        }
        CrossSpaceCallKind::Call => {
            collect_transfer_to_espace(context, EspaceTransferKind::Call, cross_space_contract)
        }
    }
}

#[derive(Clone, Copy)]
enum CrossSpaceCallKind {
    Create,
    Call,
    Withdrawal,
}

struct CrossSpaceCallContext<'a> {
    trace: &'a CommittedExecutionTrace,
    frame_id: FrameId,
    frame_position: usize,
    caller: cfx_types::Address,
    transferred_value: cfx_types::U256,
}

fn collect_transfer_to_espace(
    context: CrossSpaceCallContext<'_>,
    transfer_kind: EspaceTransferKind,
    cross_space_contract: cfx_types::Address,
) -> Result<Option<(CrossSpaceTransferOperation, Vec<usize>)>, CoreSpaceChangesError> {
    let amount = u256_from_cfx(context.transferred_value);
    let mapped_account = context.caller.evm_map();
    let mapped_sender = address_from_cfx(mapped_account.address);
    let transfer = unique_matching_transfer(&context, "transfer into eSpace", |transfer| {
        matches!(
            (transfer.from, transfer.to),
            (AddressPocket::Balance(from), AddressPocket::Balance(to))
                if transfer.space == Space::Native
                    && *from == cross_space_contract.with_native_space()
                    && *to == mapped_account
                    && transfer.value == context.transferred_value
        )
    })?;

    let child = match transfer_kind {
        EspaceTransferKind::Create => {
            let receiver =
                matching_create_event(&context, cross_space_contract, mapped_sender, amount)?;
            unique_matching_create_child(&context, mapped_account.address, receiver, amount)?
        }
        EspaceTransferKind::Call => {
            let receiver =
                matching_call_event(&context, cross_space_contract, mapped_sender, amount)?;
            unique_matching_call_child(&context, mapped_account.address, receiver, amount)?
        }
    };

    Ok(Some((
        CrossSpaceTransferOperation::ToEspace {
            position: ChangePosition::new(context.frame_position, 0),
            child_frame_id: child.frame_id,
            core_sender: address_from_cfx(context.caller),
            mapped_sender,
            receiver: child.receiver,
            amount,
        },
        vec![transfer.position, child.position],
    )))
}

#[derive(Clone, Copy)]
enum EspaceTransferKind {
    Create,
    Call,
}

fn collect_withdrawal_to_core_space(
    context: CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
) -> Result<Option<(CrossSpaceTransferOperation, Vec<usize>)>, CoreSpaceChangesError> {
    let mapped_account = context.caller.evm_map();
    let mapped_sender = address_from_cfx(mapped_account.address);
    let core_receiver = address_from_cfx(context.caller);
    let amount =
        matching_withdraw_event(&context, cross_space_contract, mapped_sender, core_receiver)?;
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
        CrossSpaceTransferOperation::ToCoreSpace {
            position: ChangePosition::new(context.frame_position, 0),
            mapped_sender,
            core_receiver,
            amount,
        },
        vec![transfer.position],
    )))
}

#[derive(Clone, Copy)]
struct MatchingChild {
    position: usize,
    frame_id: FrameId,
    receiver: Address,
}

fn unique_matching_call_child(
    context: &CrossSpaceCallContext<'_>,
    mapped_sender: cfx_types::Address,
    receiver: Address,
    amount: U256,
) -> Result<MatchingChild, CoreSpaceChangesError> {
    let children = context.trace.events().iter().filter_map(|event| {
        let TraceEvent::FrameStart { position, frame_id } = event else {
            return None;
        };
        let frame = context.trace.frame(*frame_id);
        let FrameAction::Call {
            caller,
            target,
            transferred_value,
            ..
        } = &frame.action
        else {
            return None;
        };
        (frame.parent_id == Some(context.frame_id)
            && frame.space == Space::Ethereum
            && *caller == mapped_sender
            && address_from_cfx(*target) == receiver
            && u256_from_cfx(*transferred_value) == amount)
            .then_some(MatchingChild {
                position: *position,
                frame_id: *frame_id,
                receiver,
            })
    });
    unique_matching_child(children, "call")
}

fn unique_matching_create_child(
    context: &CrossSpaceCallContext<'_>,
    mapped_sender: cfx_types::Address,
    receiver: Address,
    amount: U256,
) -> Result<MatchingChild, CoreSpaceChangesError> {
    let children = context.trace.events().iter().filter_map(|event| {
        let TraceEvent::FrameStart { position, frame_id } = event else {
            return None;
        };
        let frame = context.trace.frame(*frame_id);
        let FrameAction::Create {
            creator,
            created_address,
            value,
        } = &frame.action
        else {
            return None;
        };
        (frame.parent_id == Some(context.frame_id)
            && frame.space == Space::Ethereum
            && *creator == mapped_sender
            && address_from_cfx(*created_address) == receiver
            && u256_from_cfx(*value) == amount)
            .then_some(MatchingChild {
                position: *position,
                frame_id: *frame_id,
                receiver,
            })
    });
    unique_matching_child(children, "create")
}

fn unique_matching_child(
    mut children: impl Iterator<Item = MatchingChild>,
    action: &str,
) -> Result<MatchingChild, CoreSpaceChangesError> {
    let child = children.next().ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(format!(
            "Core cross-space {action} is missing its matching committed direct eSpace child frame"
        ))
    })?;
    if children.next().is_some() {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core cross-space {action} has ambiguous matching committed direct eSpace child frames"
        )));
    }
    Ok(child)
}

#[derive(Clone, Copy)]
struct ScopedLog<'a> {
    topics: &'a [cfx_types::H256],
    data: &'a [u8],
}

fn protocol_logs<'a>(
    context: &'a CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
) -> Vec<ScopedLog<'a>> {
    context
        .trace
        .events()
        .iter()
        .filter_map(|event| {
            let TraceEvent::Log {
                frame_id,
                address,
                topics,
                data,
                ..
            } = event
            else {
                return None;
            };
            (*frame_id == context.frame_id && *address == cross_space_contract)
                .then_some(ScopedLog { topics, data })
        })
        .collect()
}

fn matching_call_event(
    context: &CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
    sender: Address,
    value: U256,
) -> Result<Address, CoreSpaceChangesError> {
    let mut matching_receiver = None;
    for log in protocol_logs(context, cross_space_contract) {
        if log.topics.first().copied().map(b256_from_cfx) != Some(Call::SIGNATURE_HASH) {
            continue;
        }
        let event =
            Call::decode_raw_log_validate(log.topics.iter().copied().map(b256_from_cfx), log.data)
                .map_err(|error| invalid_event_data("Call", error))?;
        if address_from_bytes20(event.sender) == sender && event.value == value {
            let receiver = address_from_bytes20(event.receiver);
            if matching_receiver.replace(receiver).is_some() {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core cross-space call has ambiguous matching committed Call events",
                ));
            }
        }
    }
    matching_receiver.ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(
            "Core cross-space call is missing its matching committed Call event",
        )
    })
}

fn matching_create_event(
    context: &CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
    sender: Address,
    value: U256,
) -> Result<Address, CoreSpaceChangesError> {
    let mut matching_receiver = None;
    for log in protocol_logs(context, cross_space_contract) {
        if log.topics.first().copied().map(b256_from_cfx) != Some(Create::SIGNATURE_HASH) {
            continue;
        }
        let event = Create::decode_raw_log_validate(
            log.topics.iter().copied().map(b256_from_cfx),
            log.data,
        )
        .map_err(|error| invalid_event_data("Create", error))?;
        if address_from_bytes20(event.sender) == sender && event.value == value {
            let receiver = address_from_bytes20(event.receiver);
            if matching_receiver.replace(receiver).is_some() {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core cross-space create has ambiguous matching committed Create events",
                ));
            }
        }
    }
    matching_receiver.ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(
            "Core cross-space create is missing its matching committed Create event",
        )
    })
}

fn matching_withdraw_event(
    context: &CrossSpaceCallContext<'_>,
    cross_space_contract: cfx_types::Address,
    sender: Address,
    receiver: Address,
) -> Result<U256, CoreSpaceChangesError> {
    let mut matching_value = None;
    for log in protocol_logs(context, cross_space_contract) {
        if log.topics.first().copied().map(b256_from_cfx) != Some(Withdraw::SIGNATURE_HASH) {
            continue;
        }
        let event = Withdraw::decode_raw_log_validate(
            log.topics.iter().copied().map(b256_from_cfx),
            log.data,
        )
        .map_err(|error| invalid_event_data("Withdraw", error))?;
        if address_from_bytes20(event.sender) == sender
            && event.receiver == receiver
            && matching_value.replace(event.value).is_some()
        {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core cross-space withdrawal has ambiguous matching committed Withdraw events",
            ));
        }
    }
    matching_value.ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(
            "Core cross-space withdrawal is missing its matching committed Withdraw event",
        )
    })
}

fn invalid_event_data(name: &str, error: alloy_sol_types::Error) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core cross-space {name} event is not valid ABI data: {error}"
    ))
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
) -> Result<ScopedInternalTransfer<'a>, CoreSpaceChangesError> {
    let mut matches = context
        .trace
        .internal_transfers_in_scope(Some(context.frame_id))
        .filter_map(scoped_internal_transfer)
        .filter(|transfer| matches_transfer(*transfer));
    let transfer = matches.next().ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(format!(
            "Core cross-space {transfer_name} is missing its committed internal transfer"
        ))
    })?;
    if matches.next().is_some() {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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

fn address_from_bytes20(value: FixedBytes<20>) -> Address {
    Address::from_slice(value.as_slice())
}
