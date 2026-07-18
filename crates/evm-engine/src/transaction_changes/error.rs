use alloy_primitives::{Address, U256};
use thiserror::Error;

use super::event_codec::EventCodecError;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum TransactionChangesError {
    #[error("failed to decode event at observation {observation_index}: {source}")]
    MalformedEvent {
        observation_index: usize,
        #[source]
        source: EventCodecError,
    },

    #[error(
        "selfdestruct-to-self at observation {observation_index} burns \
           {amount} from {contract}, which cannot be represented"
    )]
    UnsupportedSelfDestructToSelf {
        observation_index: usize,
        contract: Address,
        amount: U256,
    },
}
