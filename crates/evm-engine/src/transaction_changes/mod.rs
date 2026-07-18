mod candidate;
mod collection;
mod current_changes;
mod error;
mod event_codec;

pub(crate) use candidate::{ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence};
pub(crate) use collection::collect_candidates;
pub(crate) use current_changes::{
    ContractKind, CurrentChangeFacts, Erc20Metadata, Erc721CollectionMetadata,
    build_current_changes,
};

#[cfg(test)]
mod tests;
