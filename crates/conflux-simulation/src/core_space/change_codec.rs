use contract_standards::MetadataStore;
use serde::{Serialize, Serializer};
use simulation_core::{analysis::ExecutionSpace, codec::AssetChangeRef};

use super::{CoreSpaceChange, CoreSpaceChangeSet, CrossSpaceAddress};

struct ChangeRef<'a> {
    change: &'a CoreSpaceChange,
    metadata: &'a MetadataStore<CrossSpaceAddress>,
}

impl Serialize for ChangeRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.change {
            CoreSpaceChange::Asset(change) => AssetChangeRef {
                space: Some(ExecutionSpace::Core),
                change,
                metadata: change.metadata_request().and_then(|(address, _)| {
                    self.metadata.get(&CrossSpaceAddress::CoreSpace(address))
                }),
            }
            .serialize(serializer),
            CoreSpaceChange::Espace(change) => AssetChangeRef {
                space: Some(ExecutionSpace::Espace),
                change,
                metadata: change.metadata_request().and_then(|(address, _)| {
                    self.metadata.get(&CrossSpaceAddress::Espace(address))
                }),
            }
            .serialize(serializer),
            change => {
                #[derive(Serialize)]
                struct ProtocolChange<'a> {
                    space: ExecutionSpace,
                    #[serde(flatten)]
                    change: &'a CoreSpaceChange,
                }
                ProtocolChange {
                    space: ExecutionSpace::Core,
                    change,
                }
                .serialize(serializer)
            }
        }
    }
}

impl Serialize for CoreSpaceChangeSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.items().map(|change| ChangeRef {
            change,
            metadata: self.metadata(),
        }))
    }
}
