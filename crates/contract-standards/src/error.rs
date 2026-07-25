use alloy_primitives::{Address, B256, U256};
use thiserror::Error;

use crate::{
    CollectionStandards, EventCodecError, Position, StateArithmeticOperation, StatePhase,
    StateRequirement,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContractStandardsError {
    #[error("failed to decode event at record {position:?}: {source}")]
    MalformedEvent {
        position: Position,
        #[source]
        source: EventCodecError,
    },

    #[error("{phase} state value {requirement} is missing")]
    StateValueMissing {
        requirement: StateRequirement,
        phase: StatePhase,
    },

    #[error("state {operation} failed for {requirement}: current {current}, amount {amount}")]
    StateArithmetic {
        requirement: Box<StateRequirement>,
        operation: StateArithmeticOperation,
        current: U256,
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
        "ERC-721 movement for token {token_id} in collection {collection} from {from} to {to} \
         is invalid for current owner {current_owner:?}"
    )]
    Erc721MovementInvalid {
        collection: Address,
        token_id: U256,
        from: Address,
        to: Address,
        current_owner: Option<Address>,
    },

    #[error(
        "ERC-721 approval for token {token_id} in collection {collection} by owner {event_owner} \
         is invalid for current owner {current_owner:?}"
    )]
    Erc721ApprovalInvalid {
        collection: Address,
        token_id: U256,
        event_owner: Address,
        current_owner: Option<Address>,
    },

    #[error(
        "ERC-721 owner mismatch for token {token_id} in collection {collection}: replayed \
         {replayed_owner:?}, after state {after_owner:?}"
    )]
    Erc721OwnerMismatch {
        collection: Address,
        token_id: U256,
        replayed_owner: Option<Address>,
        after_owner: Option<Address>,
    },

    #[error(
        "ERC-721 approval mismatch for token {token_id} in collection {collection}: replayed \
         {replayed_approved_address:?}, after state {after_approved_address:?}"
    )]
    Erc721ApprovalMismatch {
        collection: Address,
        token_id: U256,
        replayed_approved_address: Option<Address>,
        after_approved_address: Option<Address>,
    },

    #[error(
        "ERC-1155 transfer for token {token_id} in collection {collection} uses the zero address \
         as both sender and recipient for amount {amount}"
    )]
    Erc1155TransferBetweenZeroAddresses {
        collection: Address,
        token_id: U256,
        amount: U256,
    },

    #[error(
        "ERC-1155 balance mismatch for {account} and token {token_id} in collection {collection}: \
         replayed {replayed_balance} does not match after state"
    )]
    Erc1155BalanceMismatch {
        collection: Address,
        account: Address,
        token_id: U256,
        replayed_balance: U256,
    },

    #[error(
        "operator approval mismatch for owner {owner} and operator {operator} \
         in collection {collection}: event value {event_approved}, after state {after_approved}"
    )]
    OperatorApprovalValueMismatch {
        collection: Address,
        owner: Address,
        operator: Address,
        event_approved: bool,
        after_approved: bool,
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

    #[error("token collection {collection} does not support required standard {standard}")]
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

    #[error("token collection {collection} does not support ERC-165")]
    CollectionDoesNotSupportErc165 { collection: Address },

    #[error("token collection {collection} reports support for the invalid ERC-165 interface")]
    CollectionSupportsInvalidErc165Interface { collection: Address },
}

impl ContractStandardsError {
    pub(crate) fn state_arithmetic(
        requirement: StateRequirement,
        operation: StateArithmeticOperation,
        current: U256,
        amount: U256,
    ) -> Self {
        Self::StateArithmetic {
            requirement: Box::new(requirement),
            operation,
            current,
            amount,
        }
    }
}
