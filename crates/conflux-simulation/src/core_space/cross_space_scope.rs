use alloy_sol_types::{SolCall, SolEvent, sol};
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::{AddressSpaceUtil, AddressUtil, Space};
use cfx_vm_types::CallType;

use crate::{
    core_space::CoreSpaceChangesError,
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
};

sol! {
    function createEVM(bytes init);
    function transferEVM(bytes20 receiver);
    function callEVM(bytes20 receiver, bytes data);
    function withdrawFromMapped(uint256 value);
    event Call(bytes20 indexed sender, bytes20 indexed receiver, uint256 value, uint256 nonce, bytes data);
    event Create(bytes20 indexed sender, bytes20 indexed receiver, uint256 value, uint256 nonce, bytes data);
    event Withdraw(bytes20 indexed sender, address indexed receiver, uint256 value, uint256 nonce);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrossSpaceOperation {
    WithdrawToCore,
    CreateEspace,
    CallEspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommittedCrossSpaceTransfer {
    ToEspace {
        position: usize,
        parent_frame_id: FrameId,
        child_frame_id: FrameId,
        core_sender: cfx_types::Address,
        mapped_sender: cfx_types::Address,
        receiver: alloy_primitives::Address,
        amount: alloy_primitives::U256,
    },
    ToCoreSpace {
        position: usize,
        parent_frame_id: FrameId,
        mapped_sender: cfx_types::Address,
        core_receiver: cfx_types::Address,
        amount: alloy_primitives::U256,
    },
}

#[derive(Debug, Default)]
pub(crate) struct CommittedCrossSpaceScopes {
    pub(crate) roots: Vec<FrameId>,
    pub(crate) transfers: Vec<CommittedCrossSpaceTransfer>,
}

/// Collect committed nested eSpace scopes and their canonical value movements.
/// This runs while the finalized Core execution record is assembled, before
/// change rules inspect logs or child frames.
pub(crate) fn collect_committed_espace_scopes(
    trace: &CommittedExecutionTrace,
) -> Result<CommittedCrossSpaceScopes, CoreSpaceChangesError> {
    let contract = cfx_parameters::internal_contract_addresses::CROSS_SPACE_CONTRACT_ADDRESS;
    let mut roots = Vec::new();
    let mut transfers = Vec::new();

    for event in trace.events() {
        let TraceEvent::FrameStart { position, frame_id } = event else {
            continue;
        };
        let frame = trace.frame(*frame_id);
        let FrameAction::Call {
            call_type,
            caller,
            target,
            code_address,
            transferred_value,
            calldata_len,
            calldata,
            calldata_prefix,
            ..
        } = &frame.action
        else {
            continue;
        };
        if frame.space != Space::Native
            || *call_type != CallType::Call
            || *target != contract
            || *code_address != contract
        {
            continue;
        }
        let Some(selector) = calldata_prefix.get(..4).filter(|_| *calldata_len >= 4) else {
            continue;
        };
        let mut expected_withdraw_value = None;
        let mut expected_receiver = None;
        let mut expected_call_data = None;
        let operation = if selector == withdrawFromMappedCall::SELECTOR {
            expected_withdraw_value = Some(
                decode_calldata::<withdrawFromMappedCall>(calldata, "withdrawFromMapped")?.value,
            );
            CrossSpaceOperation::WithdrawToCore
        } else if selector == createEVMCall::SELECTOR {
            let call = decode_calldata::<createEVMCall>(calldata, "createEVM")?;
            expected_call_data = Some(call.init.to_vec());
            CrossSpaceOperation::CreateEspace
        } else if selector == transferEVMCall::SELECTOR || selector == callEVMCall::SELECTOR {
            if selector == transferEVMCall::SELECTOR {
                let call = decode_calldata::<transferEVMCall>(calldata, "transferEVM")?;
                expected_receiver = Some(address_from_bytes20(call.receiver));
            } else {
                let call = decode_calldata::<callEVMCall>(calldata, "callEVM")?;
                expected_receiver = Some(address_from_bytes20(call.receiver));
                expected_call_data = Some(call.data.to_vec());
            }
            CrossSpaceOperation::CallEspace
        } else {
            continue;
        };

        let mapped = caller.evm_map();
        let bridge_transfers: Vec<_> = trace
            .internal_transfers_in_scope(Some(*frame_id))
            .filter_map(|event| {
                let TraceEvent::InternalTransfer {
                    space: Space::Native,
                    from,
                    to,
                    value,
                    ..
                } = event
                else {
                    return None;
                };
                let matched = match operation {
                    CrossSpaceOperation::WithdrawToCore => matches!((from, to),
                        (AddressPocket::Balance(from), AddressPocket::Balance(to))
                            if *from == mapped && *to == caller.with_native_space()),
                    CrossSpaceOperation::CreateEspace | CrossSpaceOperation::CallEspace => {
                        matches!((from, to),
                        (AddressPocket::Balance(from), AddressPocket::Balance(to))
                            if *from == contract.with_native_space()
                                && *to == mapped
                                && *value == *transferred_value)
                    }
                };
                matched.then_some(*value)
            })
            .collect();
        let parent_transfer_count = trace.internal_transfers_in_scope(Some(*frame_id)).count();
        if bridge_transfers.len() != 1 || parent_transfer_count != 1 {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core cross-space bridge touch is missing, ambiguous, or accompanied by an unexpected parent transfer",
            ));
        }

        if operation == CrossSpaceOperation::WithdrawToCore {
            let amount = crate::primitive::u256_from_cfx(bridge_transfers[0]);
            unique_withdraw_event(trace, *frame_id, mapped.address, *caller, amount)?;
            if Some(amount) != expected_withdraw_value {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core cross-space withdrawal value does not match its calldata",
                ));
            }
            transfers.push(CommittedCrossSpaceTransfer::ToCoreSpace {
                position: *position,
                parent_frame_id: *frame_id,
                mapped_sender: mapped.address,
                core_receiver: *caller,
                amount,
            });
            continue;
        }

        let matching_children: Vec<_> = trace
            .events()
            .iter()
            .filter_map(|event| {
                let TraceEvent::FrameStart {
                    frame_id: child_id, ..
                } = event
                else {
                    return None;
                };
                let child = trace.frame(*child_id);
                if child.parent_id != Some(*frame_id) || child.space != Space::Ethereum {
                    return None;
                }
                let matched = match (&child.action, operation) {
                    (
                        FrameAction::Call {
                            call_type: child_call_type,
                            caller,
                            ..
                        },
                        CrossSpaceOperation::CallEspace,
                    ) => *child_call_type == CallType::Call && *caller == mapped.address,
                    (FrameAction::Create { creator, .. }, CrossSpaceOperation::CreateEspace) => {
                        *creator == mapped.address
                    }
                    _ => false,
                };
                matched.then_some(*child_id)
            })
            .collect();
        if matching_children.len() != 1 {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core cross-space operation has a missing or ambiguous eSpace child",
            ));
        }
        let child = matching_children[0];
        let (child_receiver, child_value) = match &trace.frame(child).action {
            FrameAction::Call {
                target,
                transferred_value,
                ..
            } => (
                alloy_primitives::Address::from_slice(target.as_bytes()),
                crate::primitive::u256_from_cfx(*transferred_value),
            ),
            FrameAction::Create {
                created_address,
                value,
                ..
            } => (
                alloy_primitives::Address::from_slice(created_address.as_bytes()),
                crate::primitive::u256_from_cfx(*value),
            ),
        };
        if child_value != crate::primitive::u256_from_cfx(*transferred_value) {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core cross-space child value does not match the bridge transfer",
            ));
        }
        if expected_receiver.is_some_and(|receiver| receiver != child_receiver) {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core cross-space calldata receiver does not match the eSpace child",
            ));
        }
        if let Some(expected_data) = expected_call_data.as_deref() {
            let child_data = match &trace.frame(child).action {
                FrameAction::Call { calldata, .. } => calldata.as_slice(),
                FrameAction::Create { init_code, .. } => init_code.as_slice(),
            };
            if child_data != expected_data {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core cross-space calldata payload does not match the eSpace child",
                ));
            }
        }
        match operation {
            CrossSpaceOperation::CreateEspace => {
                unique_create_event(
                    trace,
                    *frame_id,
                    mapped.address,
                    *transferred_value,
                    child_receiver,
                )?;
            }
            CrossSpaceOperation::CallEspace => {
                unique_call_event(
                    trace,
                    *frame_id,
                    mapped.address,
                    *transferred_value,
                    child_receiver,
                )?;
            }
            CrossSpaceOperation::WithdrawToCore => unreachable!(),
        }
        if roots.iter().any(|root| {
            *root == child
                || trace.frame_is_within(child, *root)
                || trace.frame_is_within(*root, child)
        }) {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core cross-space eSpace scopes overlap",
            ));
        }
        roots.push(child);
        transfers.push(CommittedCrossSpaceTransfer::ToEspace {
            position: *position,
            parent_frame_id: *frame_id,
            child_frame_id: child,
            core_sender: *caller,
            mapped_sender: mapped.address,
            receiver: child_receiver,
            amount: child_value,
        });
    }

    for event in trace.events() {
        let TraceEvent::FrameStart { frame_id, .. } = event else {
            continue;
        };
        if trace.frame(*frame_id).space == Space::Ethereum
            && !roots
                .iter()
                .any(|root| trace.frame_is_within(*frame_id, *root))
        {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "committed eSpace frame is outside a verified Core cross-space scope",
            ));
        }
    }
    Ok(CommittedCrossSpaceScopes { roots, transfers })
}

fn unique_call_event(
    trace: &CommittedExecutionTrace,
    frame_id: FrameId,
    sender: cfx_types::Address,
    value: cfx_types::U256,
    receiver: alloy_primitives::Address,
) -> Result<Call, CoreSpaceChangesError> {
    let mut matches = Vec::new();
    for event in trace.events() {
        let TraceEvent::Log {
            frame_id: id,
            address,
            topics,
            data,
            ..
        } = event
        else {
            continue;
        };
        if *id != frame_id
            || *address != cfx_parameters::internal_contract_addresses::CROSS_SPACE_CONTRACT_ADDRESS
            || topics.first().copied().map(crate::primitive::b256_from_cfx)
                != Some(Call::SIGNATURE_HASH)
        {
            continue;
        }
        let event = Call::decode_raw_log_validate(
            topics.iter().copied().map(crate::primitive::b256_from_cfx),
            data,
        )
        .map_err(|error| invalid_protocol_log("Call", error))?;
        if address_from_bytes20(event.sender) == crate::primitive::address_from_cfx(sender)
            && event.value == crate::primitive::u256_from_cfx(value)
            && address_from_bytes20(event.receiver) == receiver
        {
            matches.push(event);
        }
    }
    unique_protocol_event(matches, "call")
}

fn unique_create_event(
    trace: &CommittedExecutionTrace,
    frame_id: FrameId,
    sender: cfx_types::Address,
    value: cfx_types::U256,
    receiver: alloy_primitives::Address,
) -> Result<Create, CoreSpaceChangesError> {
    let mut matches = Vec::new();
    for event in trace.events() {
        let TraceEvent::Log {
            frame_id: id,
            address,
            topics,
            data,
            ..
        } = event
        else {
            continue;
        };
        if *id != frame_id
            || *address != cfx_parameters::internal_contract_addresses::CROSS_SPACE_CONTRACT_ADDRESS
            || topics.first().copied().map(crate::primitive::b256_from_cfx)
                != Some(Create::SIGNATURE_HASH)
        {
            continue;
        }
        let event = Create::decode_raw_log_validate(
            topics.iter().copied().map(crate::primitive::b256_from_cfx),
            data,
        )
        .map_err(|error| invalid_protocol_log("Create", error))?;
        if address_from_bytes20(event.sender) == crate::primitive::address_from_cfx(sender)
            && event.value == crate::primitive::u256_from_cfx(value)
            && address_from_bytes20(event.receiver) == receiver
        {
            matches.push(event);
        }
    }
    unique_protocol_event(matches, "create")
}

fn unique_withdraw_event(
    trace: &CommittedExecutionTrace,
    frame_id: FrameId,
    sender: cfx_types::Address,
    receiver: cfx_types::Address,
    value: alloy_primitives::U256,
) -> Result<Withdraw, CoreSpaceChangesError> {
    let mut matches = Vec::new();
    for event in trace.events() {
        let TraceEvent::Log {
            frame_id: id,
            address,
            topics,
            data,
            ..
        } = event
        else {
            continue;
        };
        if *id != frame_id
            || *address != cfx_parameters::internal_contract_addresses::CROSS_SPACE_CONTRACT_ADDRESS
            || topics.first().copied().map(crate::primitive::b256_from_cfx)
                != Some(Withdraw::SIGNATURE_HASH)
        {
            continue;
        }
        let event = Withdraw::decode_raw_log_validate(
            topics.iter().copied().map(crate::primitive::b256_from_cfx),
            data,
        )
        .map_err(|error| invalid_protocol_log("Withdraw", error))?;
        if address_from_bytes20(event.sender) == crate::primitive::address_from_cfx(sender)
            && event.receiver == crate::primitive::address_from_cfx(receiver)
            && event.value == value
        {
            matches.push(event);
        }
    }
    unique_protocol_event(matches, "withdrawal")
}

fn decode_calldata<C: SolCall>(
    calldata: &[u8],
    operation: &str,
) -> Result<C, CoreSpaceChangesError> {
    C::abi_decode(calldata).map_err(|error| {
        CoreSpaceChangesError::inconsistent_execution(format!(
            "Core cross-space {operation} calldata is malformed: {error}"
        ))
    })
}

fn unique_protocol_event<T>(events: Vec<T>, operation: &str) -> Result<T, CoreSpaceChangesError> {
    match events.len() {
        1 => events.into_iter().next().ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core cross-space {operation} is missing its matching protocol log"
            ))
        }),
        0 => Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core cross-space {operation} is missing its matching protocol log"
        ))),
        _ => Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core cross-space {operation} has ambiguous matching protocol logs"
        ))),
    }
}

fn invalid_protocol_log(name: &str, error: alloy_sol_types::Error) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core cross-space {name} protocol log is malformed: {error}"
    ))
}

fn address_from_bytes20(value: alloy_primitives::FixedBytes<20>) -> alloy_primitives::Address {
    alloy_primitives::Address::from_slice(value.as_slice())
}
