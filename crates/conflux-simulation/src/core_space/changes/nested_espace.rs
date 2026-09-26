use super::cross_space::CommittedCrossSpaceScopes;
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::Space;

use crate::{
    core_space::{
        CoreSpaceAnalysisError, CoreSpaceChangeSet, CoreSpaceChangeSetBuilder,
        CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceProtocolError,
    },
    espace::{EspaceChange, EspaceNativeCurrency},
    execution::{FrameAction, TraceEvent},
    primitive::u256_from_cfx,
};

pub(super) fn derive_native_changes(
    execution: &CoreSpaceExecutedTransaction,
    currency: &EspaceNativeCurrency,
    cross_space: &CommittedCrossSpaceScopes,
) -> Result<CoreSpaceChangeSet, CoreSpaceAnalysisError> {
    let trace = execution.trace();
    let roots = &cross_space.roots;
    let in_scope = |frame_id| {
        roots
            .iter()
            .any(|root| trace.frame_is_within(frame_id, *root))
    };
    let mut builder = CoreSpaceChangeSetBuilder::new();

    for event in trace.events() {
        match event {
            TraceEvent::FrameStart { position, frame_id } if in_scope(*frame_id) => {
                // The bridge report already includes the root child's value.
                if cross_space.owns_value_transfer(*frame_id) {
                    continue;
                }
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
                        builder.espace(
                            CoreSpaceExecutionPosition::from_index(*position),
                            EspaceChange::NativeTransfer(
                                simulation_core::changes::NativeTransfer {
                                    from: crate::primitive::address_from_cfx(*caller),
                                    to: crate::primitive::address_from_cfx(*target),
                                    raw_amount: amount,
                                    currency: currency.clone(),
                                },
                            ),
                        );
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
                        builder.espace(
                            CoreSpaceExecutionPosition::from_index(*position),
                            EspaceChange::NativeTransfer(
                                simulation_core::changes::NativeTransfer {
                                    from: crate::primitive::address_from_cfx(*creator),
                                    to: crate::primitive::address_from_cfx(*created),
                                    raw_amount: amount,
                                    currency: currency.clone(),
                                },
                            ),
                        );
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
                builder.espace(
                    CoreSpaceExecutionPosition::from_index(*position),
                    EspaceChange::NativeTransfer(simulation_core::changes::NativeTransfer {
                        from: crate::primitive::address_from_cfx(from.address),
                        to: crate::primitive::address_from_cfx(to.address),
                        raw_amount: amount,
                        currency: currency.clone(),
                    }),
                );
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
                builder.espace(
                    CoreSpaceExecutionPosition::from_index(*position),
                    EspaceChange::SelfDestructBurn(simulation_core::changes::NativeBurn {
                        contract_address: crate::primitive::address_from_cfx(from.address),
                        raw_amount: amount,
                        currency: currency.clone(),
                    }),
                );
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
                builder.espace(
                    CoreSpaceExecutionPosition::from_index(*position),
                    EspaceChange::SelfDestructBurn(simulation_core::changes::NativeBurn {
                        contract_address: crate::primitive::address_from_cfx(from.address),
                        raw_amount: amount,
                        currency: currency.clone(),
                    }),
                );
            }
            _ => {}
        }
    }

    Ok(builder.finish())
}
