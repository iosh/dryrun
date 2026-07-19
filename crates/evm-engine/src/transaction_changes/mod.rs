mod candidate;
mod change_data;
mod collection;
mod current_changes;
mod error;
mod event_codec;
mod native_balance;
mod token_contract;
mod token_state;

pub(crate) use candidate::ChangeCandidate;
pub(crate) use change_data::{
    ChangeData, ChangeDataRequests, ContractKind, Erc20Metadata, Erc721CollectionMetadata,
    collect_change_data_requests,
};
pub(crate) use collection::collect_candidates;
pub(crate) use current_changes::build_changes;
pub(crate) use native_balance::check_native_balances;
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
