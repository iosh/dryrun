use contract_standards::TokenMetadata;
use serde::{Serialize, Serializer};

use crate::{
    analysis::ExecutionSpace,
    changes::{AssetChange, AssetChangeSet},
};

/// Display fields borrow the one metadata record held by the result.
#[derive(Serialize)]
pub struct AssetChangeRef<'a, A> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space: Option<ExecutionSpace>,
    #[serde(flatten)]
    pub change: &'a AssetChange<A>,
    #[serde(flatten)]
    pub metadata: Option<&'a TokenMetadata>,
}

impl<A: Ord + Clone + Serialize> Serialize for AssetChangeSet<A> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.items().map(|change| {
            AssetChangeRef {
                space: self.space(),
                change,
                metadata: change
                    .metadata_request()
                    .and_then(|(address, _)| self.metadata().get(&address)),
            }
        }))
    }
}
