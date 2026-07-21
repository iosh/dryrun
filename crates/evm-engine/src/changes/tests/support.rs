use alloy_primitives::{Address, U256};

use super::super::candidate::{ChangeCandidate, ChangeCandidateKind, ObservationPosition};

pub(super) fn candidate(
    observation_index: usize,
    item_index: usize,
    kind: ChangeCandidateKind,
) -> ChangeCandidate {
    ChangeCandidate {
        position: ObservationPosition {
            observation_index,
            item_index,
        },
        kind,
    }
}

pub(super) fn native_candidate(
    observation_index: usize,
    from: Address,
    to: Address,
    amount: U256,
) -> ChangeCandidate {
    candidate(
        observation_index,
        0,
        ChangeCandidateKind::NativeTransfer { from, to, amount },
    )
}

pub(super) fn erc20_movement_candidate(
    observation_index: usize,
    token: Address,
    from: Address,
    to: Address,
    amount: U256,
) -> ChangeCandidate {
    candidate(
        observation_index,
        0,
        ChangeCandidateKind::Erc20Movement {
            token,
            from,
            to,
            amount,
        },
    )
}
