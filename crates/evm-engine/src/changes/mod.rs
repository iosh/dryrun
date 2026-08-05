mod contract_candidates;

pub(crate) use contract_candidates::collect_contract_candidates;
pub(crate) use simulation_changes::{
    ChangeMetadata, PositionedChange, into_enriched_changes, sort_changes_by_position,
};

#[cfg(test)]
mod tests;
