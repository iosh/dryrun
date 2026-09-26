use std::collections::BTreeMap;

use cfx_executor::executive_observer::AddressPocket;
use cfx_types::Space;

use crate::{
    core_space::{
        CoreSpaceAnalysisError, CoreSpaceChangeSet, CoreSpaceChangeSetBuilder,
        CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceProtocolError,
        CoreSpaceStateAccess,
    },
    espace::{EspaceChange, EspaceNativeCurrency, has_nested_token_logs},
    execution::{FrameAction, TraceEvent},
    primitive::{u256_from_cfx, u256_to_cfx},
};

#[derive(Default)]
struct BalanceDelta {
    credited: cfx_types::U256,
    debited: cfx_types::U256,
}

pub(super) fn derive_native_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    currency: &EspaceNativeCurrency,
    wrapped_native_token: alloy_primitives::Address,
) -> Result<CoreSpaceChangeSet, CoreSpaceAnalysisError> {
    let trace = execution.trace();
    let roots = execution.nested_espace_scope_roots();
    let has_token_logs =
        has_nested_token_logs(trace, roots, wrapped_native_token).map_err(|error| {
            CoreSpaceAnalysisError::Protocol(CoreSpaceProtocolError::inconsistent_execution(
                format!("nested eSpace token occurrence is invalid: {error}"),
            ))
        })?;
    if has_token_logs {
        return Err(CoreSpaceAnalysisError::Protocol(
            CoreSpaceProtocolError::unsupported_operation(
                "nested eSpace token changes require a dedicated eSpace state view",
            ),
        ));
    }

    let in_scope = |frame_id| {
        roots
            .iter()
            .any(|root| trace.frame_is_within(frame_id, *root))
    };
    let mut deltas = BTreeMap::<_, BalanceDelta>::new();
    let mut builder = CoreSpaceChangeSetBuilder::new();

    for event in trace.events() {
        match event {
            TraceEvent::FrameStart { position, frame_id } if in_scope(*frame_id) => {
                let frame = trace.frame(*frame_id);
                match &frame.action {
                    FrameAction::Call {
                        call_type: cfx_vm_types::CallType::Call,
                        caller,
                        target,
                        transferred_value,
                        ..
                    } if !transferred_value.is_zero() => {
                        let amount = u256_from_cfx(*transferred_value);
                        record_transfer(&mut deltas, caller, target, u256_to_cfx(amount))
                            .map_err(CoreSpaceAnalysisError::Protocol)?;
                        builder.espace(
                            CoreSpaceExecutionPosition::from_index(*position),
                            EspaceChange::NativeTransfer {
                                from: crate::primitive::address_from_cfx(*caller),
                                to: crate::primitive::address_from_cfx(*target),
                                raw_amount: amount,
                                currency: currency.clone(),
                            },
                        )?;
                    }
                    FrameAction::Call {
                        call_type,
                        transferred_value,
                        ..
                    } if !transferred_value.is_zero() => {
                        return Err(CoreSpaceAnalysisError::Protocol(
                            CoreSpaceProtocolError::inconsistent_execution(format!(
                                "nonzero nested eSpace {call_type:?} value is not a balance transfer"
                            )),
                        ));
                    }
                    FrameAction::Create {
                        creator,
                        actual_created_address: Some(created),
                        value,
                        ..
                    } if !value.is_zero() => {
                        let amount = u256_from_cfx(*value);
                        record_transfer(&mut deltas, creator, created, u256_to_cfx(amount))
                            .map_err(CoreSpaceAnalysisError::Protocol)?;
                        builder.espace(
                            CoreSpaceExecutionPosition::from_index(*position),
                            EspaceChange::NativeTransfer {
                                from: crate::primitive::address_from_cfx(*creator),
                                to: crate::primitive::address_from_cfx(*created),
                                raw_amount: amount,
                                currency: currency.clone(),
                            },
                        )?;
                    }
                    FrameAction::Create { value, .. } if !value.is_zero() => {
                        return Err(CoreSpaceAnalysisError::Protocol(
                            CoreSpaceProtocolError::inconsistent_execution(
                                "committed eSpace create is missing its actual address",
                            ),
                        ));
                    }
                    _ => {}
                }
            }
            TraceEvent::InternalTransfer {
                position,
                space: Space::Ethereum,
                from: AddressPocket::Balance(from),
                to: AddressPocket::Balance(to),
                value,
                frame_id: Some(frame_id),
            } if in_scope(*frame_id)
                && from.space == Space::Ethereum
                && to.space == Space::Ethereum =>
            {
                if value.is_zero() {
                    continue;
                }
                let amount = u256_from_cfx(*value);
                record_transfer(&mut deltas, &from.address, &to.address, u256_to_cfx(amount))
                    .map_err(CoreSpaceAnalysisError::Protocol)?;
                builder.espace(
                    CoreSpaceExecutionPosition::from_index(*position),
                    EspaceChange::NativeTransfer {
                        from: crate::primitive::address_from_cfx(from.address),
                        to: crate::primitive::address_from_cfx(to.address),
                        raw_amount: amount,
                        currency: currency.clone(),
                    },
                )?;
            }
            TraceEvent::InternalTransfer {
                position,
                space: Space::Native,
                from: AddressPocket::Balance(from),
                to: AddressPocket::MintBurn,
                value,
                frame_id: None,
            } if from.space == Space::Ethereum => {
                if value.is_zero() {
                    continue;
                }
                let amount = u256_from_cfx(*value);
                let delta = deltas.entry(from.address).or_default();
                delta.debited = delta.debited.checked_add(*value).ok_or_else(|| {
                    CoreSpaceAnalysisError::Protocol(
                        CoreSpaceProtocolError::inconsistent_execution(
                            "nested eSpace balance delta overflows",
                        ),
                    )
                })?;
                builder.espace(
                    CoreSpaceExecutionPosition::from_index(*position),
                    EspaceChange::SelfDestructBurn {
                        contract_address: crate::primitive::address_from_cfx(from.address),
                        raw_amount: amount,
                        currency: currency.clone(),
                    },
                )?;
            }
            TraceEvent::InternalTransfer {
                position,
                space: Space::Ethereum,
                from: AddressPocket::Balance(from),
                to: AddressPocket::MintBurn,
                value,
                frame_id: Some(frame_id),
            } if in_scope(*frame_id) && from.space == Space::Ethereum => {
                if value.is_zero() {
                    continue;
                }
                let amount = u256_from_cfx(*value);
                let delta = deltas.entry(from.address).or_default();
                delta.debited = delta.debited.checked_add(*value).ok_or_else(|| {
                    CoreSpaceAnalysisError::Protocol(
                        CoreSpaceProtocolError::inconsistent_execution(
                            "nested eSpace balance delta overflows",
                        ),
                    )
                })?;
                builder.espace(
                    CoreSpaceExecutionPosition::from_index(*position),
                    EspaceChange::SelfDestructBurn {
                        contract_address: crate::primitive::address_from_cfx(from.address),
                        raw_amount: amount,
                        currency: currency.clone(),
                    },
                )?;
            }
            _ => {}
        }
    }

    for (address, delta) in deltas {
        let before = state.initial().espace_balance(address).map_err(|error| {
            CoreSpaceAnalysisError::Protocol(CoreSpaceProtocolError::state_access(
                "read nested eSpace initial balance",
                error,
            ))
        })?;
        let after = state.finalized().espace_balance(address).map_err(|error| {
            CoreSpaceAnalysisError::Protocol(CoreSpaceProtocolError::state_access(
                "read nested eSpace finalized balance",
                error,
            ))
        })?;
        let expected = before
            .checked_add(delta.credited)
            .and_then(|value| value.checked_sub(delta.debited))
            .ok_or_else(|| {
                CoreSpaceAnalysisError::Protocol(CoreSpaceProtocolError::inconsistent_execution(
                    "nested eSpace balance delta underflows",
                ))
            })?;
        if expected != after {
            return Err(CoreSpaceAnalysisError::Protocol(
                CoreSpaceProtocolError::inconsistent_execution(
                    "nested eSpace native balance delta does not match finalized state",
                ),
            ));
        }
    }
    Ok(builder.finish())
}

fn record_transfer(
    deltas: &mut BTreeMap<cfx_types::Address, BalanceDelta>,
    from: &cfx_types::Address,
    to: &cfx_types::Address,
    amount: cfx_types::U256,
) -> Result<(), CoreSpaceProtocolError> {
    let from_delta = deltas.entry(*from).or_default();
    from_delta.debited = from_delta.debited.checked_add(amount).ok_or_else(|| {
        CoreSpaceProtocolError::inconsistent_execution("nested eSpace balance delta overflows")
    })?;
    let to_delta = deltas.entry(*to).or_default();
    to_delta.credited = to_delta.credited.checked_add(amount).ok_or_else(|| {
        CoreSpaceProtocolError::inconsistent_execution("nested eSpace balance delta overflows")
    })?;
    Ok(())
}
