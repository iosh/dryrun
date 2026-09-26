use alloy::primitives::{Address, U256};

use super::{EvmCallKind, EvmExecutionPosition, EvmFrameAction, events::EvmExecutionObservation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeMovement {
    Transfer {
        position: EvmExecutionPosition,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        position: EvmExecutionPosition,
        contract: Address,
        amount: U256,
    },
}

pub(super) fn collect_movements(observation: &EvmExecutionObservation) -> Vec<NativeMovement> {
    let mut operations = Vec::new();

    for frame in &observation.frames {
        match frame.action() {
            EvmFrameAction::Call {
                kind: EvmCallKind::Call,
                caller,
                target,
                value,
                ..
            } if !value.is_zero() && caller != target => {
                operations.push(NativeMovement::Transfer {
                    position: frame.position(),
                    from: *caller,
                    to: *target,
                    amount: *value,
                });
            }
            EvmFrameAction::Create {
                caller,
                value,
                created_address,
                ..
            } if !value.is_zero() => {
                let to = created_address.unwrap_or_else(|| {
                    unreachable!(
                        "successful CREATE frame must have an address after execution commit"
                    )
                });
                if *caller != to {
                    operations.push(NativeMovement::Transfer {
                        position: frame.position(),
                        from: *caller,
                        to,
                        amount: *value,
                    });
                }
            }
            EvmFrameAction::Call { .. } | EvmFrameAction::Create { .. } => {}
        }
    }

    for selfdestruct in &observation.selfdestructs {
        let amount = selfdestruct.value();
        if amount.is_zero()
            || (selfdestruct.contract() == selfdestruct.target()
                && !selfdestruct.destroys_contract())
        {
            continue;
        }

        if selfdestruct.contract() == selfdestruct.target() {
            operations.push(NativeMovement::SelfDestructBurn {
                position: selfdestruct.position(),
                contract: selfdestruct.contract(),
                amount,
            });
        } else {
            operations.push(NativeMovement::Transfer {
                position: selfdestruct.position(),
                from: selfdestruct.contract(),
                to: selfdestruct.target(),
                amount,
            });
        }
    }

    operations.sort_by_key(|operation| match operation {
        NativeMovement::Transfer { position, .. }
        | NativeMovement::SelfDestructBurn { position, .. } => *position,
    });
    operations
}
