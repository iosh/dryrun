use crate::Change;

use super::candidate::ObservationPosition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositionedChange {
    pub(super) position: ObservationPosition,
    pub(super) change: Change,
}

impl PositionedChange {
    pub(super) fn new(position: ObservationPosition, change: Change) -> Self {
        Self { position, change }
    }
}
