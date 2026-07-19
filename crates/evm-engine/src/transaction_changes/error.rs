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

    #[error("ERC-20 {state} balance for {account} in token {token} is missing")]
    Erc20BalanceMissing {
        token: Address,
        account: Address,
        state: &'static str,
    },

    #[error(
        "ERC-20 balance underflow for {account} in token {token}: \
       balance {balance}, cannot subtract {amount}"
    )]
    Erc20BalanceUnderflow {
        token: Address,
        account: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "ERC-20 balance overflow for {account} in token {token}: \
       balance {balance}, cannot add {amount}"
    )]
    Erc20BalanceOverflow {
        token: Address,
        account: Address,
        balance: U256,
        amount: U256,
    },
    #[error("ERC-20 {state} total supply for token {token} is missing")]
    Erc20TotalSupplyMissing { token: Address, state: &'static str },

    #[error(
        "ERC-20 total supply underflow for token {token}: \
       total supply {total_supply}, cannot subtract {amount}"
    )]
    Erc20TotalSupplyUnderflow {
        token: Address,
        total_supply: U256,
        amount: U256,
    },

    #[error(
        "ERC-20 total supply overflow for token {token}: \
       total supply {total_supply}, cannot add {amount}"
    )]
    Erc20TotalSupplyOverflow {
        token: Address,
        total_supply: U256,
        amount: U256,
    },

    #[error(
        "ERC-20 transfer for token {token} uses the zero address as both \
       sender and recipient for amount {amount}"
    )]
    Erc20TransferBetweenZeroAddresses { token: Address, amount: U256 },

    #[error(
        "ERC-20 balance mismatch for {account} in token {token}: \
       replayed {replayed_balance}, after state {after_balance}"
    )]
    Erc20BalanceMismatch {
        token: Address,
        account: Address,
        replayed_balance: U256,
        after_balance: U256,
    },

    #[error(
        "ERC-20 total supply mismatch for token {token}: \
       replayed {replayed_total_supply}, after state {after_total_supply}"
    )]
    Erc20TotalSupplyMismatch {
        token: Address,
        replayed_total_supply: U256,
        after_total_supply: U256,
    },

    #[error(
        "ERC-20 {state} allowance for owner {owner} and spender {spender} \
       in token {token} is missing"
    )]
    Erc20AllowanceMissing {
        token: Address,
        owner: Address,
        spender: Address,
        state: &'static str,
    },

    #[error(
        "ERC-20 allowance evidence for owner {owner} and spender {spender} \
       in token {token} is missing"
    )]
    Erc20AllowanceEvidenceMissing {
        token: Address,
        owner: Address,
        spender: Address,
    },

    #[error(
        "ERC-20 approval value mismatch for owner {owner} and spender {spender} \
       in token {token}: event value {event_value}, after state {after_allowance}"
    )]
    Erc20ApprovalValueMismatch {
        token: Address,
        owner: Address,
        spender: Address,
        event_value: U256,
        after_allowance: U256,
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
