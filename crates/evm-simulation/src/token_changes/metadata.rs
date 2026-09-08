use std::collections::HashSet;

use alloy::primitives::Address;
use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, MetadataCall, MetadataValues, metadata_calls,
};

use crate::state::{EvmReadCallOutcome, EvmStateAccess};

use super::events::ObservedTokenEvent;

pub(super) struct TokenMetadataOutcomes {
    values: MetadataValues<Address>,
}

impl TokenMetadataOutcomes {
    pub(super) fn erc20(&self, contract: &Address) -> Erc20Metadata {
        self.values
            .erc20_metadata(contract)
            .unwrap_or_else(|_| unreachable!("token metadata collection records every outcome"))
    }

    pub(super) fn erc721(&self, collection: &Address) -> Erc721CollectionMetadata {
        self.values
            .erc721_collection_metadata(collection)
            .unwrap_or_else(|_| unreachable!("token metadata collection records every outcome"))
    }
}

pub(super) fn load_metadata(
    events: &[ObservedTokenEvent],
    state: &EvmStateAccess,
) -> TokenMetadataOutcomes {
    let decoded = events.iter().filter_map(|event| match event {
        ObservedTokenEvent::Standard { decoded, .. } => Some(decoded),
        ObservedTokenEvent::Wrapped { .. } => None,
    });
    let mut calls = metadata_calls(decoded);
    let mut seen = calls.iter().cloned().collect::<HashSet<_>>();
    for event in events {
        let ObservedTokenEvent::Wrapped { contract, .. } = event else {
            continue;
        };
        for call in [
            MetadataCall::Name {
                contract_address: *contract,
            },
            MetadataCall::Symbol {
                contract_address: *contract,
            },
            MetadataCall::Decimals {
                contract_address: *contract,
            },
        ] {
            if seen.insert(call.clone()) {
                calls.push(call);
            }
        }
    }

    let mut values = MetadataValues::default();
    for call in calls {
        let target = *call.contract_address();
        match state.finalized().read_call(target, call.call_data()) {
            Ok(EvmReadCallOutcome::Success(output)) => {
                values.record_output(call, &output);
            }
            Ok(_) | Err(_) => values.record_unavailable(call),
        }
    }
    TokenMetadataOutcomes { values }
}
