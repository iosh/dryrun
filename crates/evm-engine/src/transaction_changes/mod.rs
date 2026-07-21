mod candidate;
mod change_metadata;
mod changes;
mod collection;
mod erc1155;
mod erc20;
mod erc721;
mod error;
mod event_codec;
mod native_balance;
mod operator_approval;
mod positioned_change;
mod token_contract;
mod token_state;

pub(crate) use change_metadata::{
    ChangeMetadata, ChangeMetadataRequests, collect_change_metadata_requests,
};
pub(crate) use changes::{build_changes, sort_changes_by_position};
pub(crate) use collection::collect_candidates;
pub(crate) use erc20::check_erc20_changes;
pub(crate) use erc721::check_erc721_changes;
pub(crate) use erc1155::check_erc1155_movements;
pub(crate) use error::TransactionChangesError;
pub(crate) use native_balance::check_native_balances;
pub(crate) use operator_approval::check_operator_approvals;
pub(crate) use positioned_change::PositionedChange;
pub(crate) use token_contract::check_token_contracts;
pub(crate) use token_state::{
    CollectionStandards, Erc721TokenKey, Erc721TokenState, TokenStateKeys, TokenStateValues,
    collect_token_state_keys,
};

#[cfg(test)]
pub(crate) use token_state::{
    Erc20AllowanceKey, Erc20BalanceKey, Erc1155BalanceKey, OperatorApprovalKey,
};

#[cfg(test)]
mod tests;
