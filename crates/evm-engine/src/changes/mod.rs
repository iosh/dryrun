use crate::Change;

mod candidate;
mod erc1155;
mod erc20;
mod erc721;
mod error;
mod event_codec;
mod metadata;
mod native;
mod observation;
mod operator_approval;
mod token_contract;
mod token_state;

use candidate::ObservationPosition;

pub(crate) use candidate::collect_candidates;
pub(crate) use erc20::check_erc20_changes;
pub(crate) use erc721::check_erc721_changes;
pub(crate) use erc1155::check_erc1155_movements;
pub(crate) use error::TransactionChangesError;
pub(crate) use metadata::{
    ChangeMetadata, ChangeMetadataRequests, collect_change_metadata_requests,
};
pub(crate) use native::check_native_balances;
pub(crate) use observation::ChangeObservationInspector;
pub(crate) use operator_approval::check_operator_approvals;
pub(crate) use token_contract::check_token_contracts;
pub(crate) use token_state::{
    CollectionStandards, Erc721TokenKey, Erc721TokenState, TokenStateKeys, TokenStateValues,
    collect_token_state_keys,
};

#[cfg(test)]
pub(crate) use token_state::{
    Erc20AllowanceKey, Erc20BalanceKey, Erc1155BalanceKey, OperatorApprovalKey,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositionedChange {
    position: ObservationPosition,
    change: Change,
}

impl PositionedChange {
    fn new(position: ObservationPosition, change: Change) -> Self {
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
