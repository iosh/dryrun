use crate::Change;

use super::{ChangeMetadata, PositionedChange};

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
