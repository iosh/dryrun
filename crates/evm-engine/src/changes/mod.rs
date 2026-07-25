use crate::Change;
use contract_standards::Position;

mod contract_candidates;
mod contract_changes;
mod error;
mod metadata;
mod native;
mod observation;

pub(crate) use contract_candidates::collect_contract_candidates;
pub(crate) use contract_changes::map_contract_changes;
pub(crate) use error::TransactionChangesError;
pub(crate) use metadata::{
    ChangeMetadata, ChangeMetadataRequests, collect_change_metadata_requests,
};
pub(crate) use native::{check_native_balances, collect_native_candidates};
pub(crate) use observation::ChangeObservationInspector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositionedChange {
    position: Position,
    change: Change,
}

impl PositionedChange {
    pub(crate) fn new(position: Position, change: Change) -> Self {
        Self { position, change }
    }
}

pub(crate) fn sort_changes_by_position(changes: &mut [PositionedChange]) {
    changes.sort_by_key(|positioned_change| positioned_change.position);
}

pub(crate) fn build_changes(
    positioned_changes: Vec<PositionedChange>,
    metadata: &ChangeMetadata,
) -> Vec<Change> {
    positioned_changes
        .into_iter()
        .map(|mut positioned_change| {
            metadata.enrich(&mut positioned_change.change);
            positioned_change.change
        })
        .collect()
}

#[cfg(test)]
mod tests;
