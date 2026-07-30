mod contract_candidates;
mod error;
mod native;
mod observation;

pub(crate) use contract_candidates::collect_contract_candidates;
pub(crate) use error::TransactionChangesError;
pub(crate) use native::{check_native_balances, collect_native_candidates};
pub(crate) use observation::ChangeObservationInspector;
pub(crate) use simulation_changes::{
    ChangeMetadata, PositionedChange, into_enriched_changes, sort_changes_by_position,
};

#[cfg(test)]
mod tests;
