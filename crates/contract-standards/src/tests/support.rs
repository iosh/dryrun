use alloy_primitives::{Address, U256};

use crate::{
    Position,
    candidate::{StandardCandidate, StandardCandidateKind},
};

pub(super) fn candidate(
    index: usize,
    item_index: usize,
    kind: StandardCandidateKind,
) -> StandardCandidate {
    StandardCandidate {
        position: Position::new(index, item_index),
        kind,
    }
}

pub(super) fn erc20_movement_candidate(
    index: usize,
    token: Address,
    from: Address,
    to: Address,
    amount: U256,
) -> StandardCandidate {
    candidate(
        index,
        0,
        StandardCandidateKind::Erc20Movement {
            token,
            from,
            to,
            amount,
        },
    )
}
