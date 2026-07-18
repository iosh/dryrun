mod candidate;
mod change_data;
mod collection;
mod current_changes;
mod error;
mod event_codec;

pub(crate) use candidate::ChangeCandidate;
pub(crate) use change_data::{
    ChangeData, ChangeDataRequests, ContractKind, Erc20Metadata, Erc721CollectionMetadata,
    collect_change_data_requests,
};
pub(crate) use collection::collect_candidates;
pub(crate) use current_changes::build_changes;

#[cfg(test)]
mod tests;
