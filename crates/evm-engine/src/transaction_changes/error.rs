use alloy_primitives::{Address, B256, U256};
use thiserror::Error;

use super::{event_codec::EventCodecError, token_state::CollectionStandards};

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

    #[error("native account {address} is missing from transaction state")]
    NativeAccountMissing { address: Address },

    #[error(
        "native balance underflow for {address}: \
           balance {balance}, cannot subtract {amount}"
    )]
    NativeBalanceUnderflow {
        address: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "native balance overflow for {address}: \
           balance {balance}, cannot add {amount}"
    )]
    NativeBalanceOverflow {
        address: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "native balance mismatch for {address}: \
           replayed {replayed_balance}, transaction state {state_balance}"
    )]
    NativeBalanceMismatch {
        address: Address,
        replayed_balance: U256,
        state_balance: U256,
    },

    #[error("token state values are missing {value} for {address}")]
    TokenStateValueMissing {
        address: Address,
        value: &'static str,
    },

    #[error(
        "token contract {contract} runtime code changed from \
       {before_code_hash} to {after_code_hash}"
    )]
    TokenContractCodeChanged {
        contract: Address,
        before_code_hash: B256,
        after_code_hash: B256,
    },

    #[error(
        "token collection {collection} standards changed from \
       {before:?} to {after:?}"
    )]
    CollectionStandardsChanged {
        collection: Address,
        before: CollectionStandards,
        after: CollectionStandards,
    },

    #[error(
        "token collection {collection} does not support required \
       standard {standard}"
    )]
    CollectionStandardNotSupported {
        collection: Address,
        standard: &'static str,
    },

    #[error(
        "operator approval collection {collection} cannot be classified uniquely: \
       ERC-721={supports_erc721}, ERC-1155={supports_erc1155}"
    )]
    OperatorApprovalStandardAmbiguous {
        collection: Address,
        supports_erc721: bool,
        supports_erc1155: bool,
    },
}
