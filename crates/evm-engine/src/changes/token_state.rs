use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use alloy_primitives::{Address, B256, U256};

use super::candidate::{ChangeCandidate, ChangeCandidateKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Erc20BalanceKey {
    pub(crate) token: Address,
    pub(crate) account: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Erc20AllowanceKey {
    pub(crate) token: Address,
    pub(crate) owner: Address,
    pub(crate) spender: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Erc721TokenKey {
    pub(crate) collection: Address,
    pub(crate) token_id: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Erc1155BalanceKey {
    pub(crate) collection: Address,
    pub(crate) account: Address,
    pub(crate) token_id: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct OperatorApprovalKey {
    pub(crate) collection: Address,
    pub(crate) owner: Address,
    pub(crate) operator: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TokenStateKeys {
    pub(crate) token_contracts: Vec<Address>,
    pub(crate) collection_standards: Vec<Address>,
    pub(crate) erc20_balances: Vec<Erc20BalanceKey>,
    pub(crate) erc20_total_supplies: Vec<Address>,
    pub(crate) erc20_allowances: Vec<Erc20AllowanceKey>,
    pub(crate) erc721_tokens: Vec<Erc721TokenKey>,
    pub(crate) erc1155_balances: Vec<Erc1155BalanceKey>,
    pub(crate) operator_approvals: Vec<OperatorApprovalKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollectionStandards {
    pub(crate) supports_erc721: bool,
    pub(crate) supports_erc1155: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Erc721TokenState {
    Present {
        owner: Address,
        approved_address: Option<Address>,
    },
    OwnerOfReverted,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TokenStateValues {
    pub(crate) contract_code_hashes: HashMap<Address, B256>,
    pub(crate) collection_standards: HashMap<Address, CollectionStandards>,
    pub(crate) erc20_balances: HashMap<Erc20BalanceKey, U256>,
    pub(crate) erc20_total_supplies: HashMap<Address, U256>,
    pub(crate) erc20_allowances: HashMap<Erc20AllowanceKey, U256>,
    pub(crate) erc721_tokens: HashMap<Erc721TokenKey, Erc721TokenState>,
    pub(crate) erc1155_balances: HashMap<Erc1155BalanceKey, U256>,
    pub(crate) operator_approvals: HashMap<OperatorApprovalKey, bool>,
}

fn retain_unique<T>(values: &mut Vec<T>)
where
    T: Copy + Eq + Hash,
{
    let mut seen = HashSet::with_capacity(values.len());
    values.retain(|value| seen.insert(*value));
}

pub(crate) fn collect_token_state_keys(candidates: &[ChangeCandidate]) -> TokenStateKeys {
    let mut keys = TokenStateKeys::default();

    for candidate in candidates {
        match candidate.kind {
            ChangeCandidateKind::NativeTransfer { .. } => {}

            ChangeCandidateKind::Erc20Movement {
                token, from, to, ..
            } => {
                keys.token_contracts.push(token);

                if from == Address::ZERO {
                    keys.erc20_total_supplies.push(token);
                } else {
                    keys.erc20_balances.push(Erc20BalanceKey {
                        token,
                        account: from,
                    });
                }

                if to == Address::ZERO {
                    keys.erc20_total_supplies.push(token);
                } else {
                    keys.erc20_balances
                        .push(Erc20BalanceKey { token, account: to });
                }
            }

            ChangeCandidateKind::Erc20Allowance {
                token,
                owner,
                spender,
                ..
            } => {
                keys.token_contracts.push(token);
                keys.erc20_allowances.push(Erc20AllowanceKey {
                    token,
                    owner,
                    spender,
                });
            }

            ChangeCandidateKind::Erc721Transfer {
                collection,
                token_id,
                ..
            }
            | ChangeCandidateKind::Erc721Approval {
                collection,
                token_id,
                ..
            } => {
                keys.token_contracts.push(collection);
                keys.collection_standards.push(collection);
                keys.erc721_tokens.push(Erc721TokenKey {
                    collection,
                    token_id,
                });
            }

            ChangeCandidateKind::Erc1155Transfer {
                collection,
                from,
                to,
                token_id,
                ..
            } => {
                keys.token_contracts.push(collection);
                keys.collection_standards.push(collection);

                if from != Address::ZERO {
                    keys.erc1155_balances.push(Erc1155BalanceKey {
                        collection,
                        account: from,
                        token_id,
                    });
                }

                if to != Address::ZERO {
                    keys.erc1155_balances.push(Erc1155BalanceKey {
                        collection,
                        account: to,
                        token_id,
                    });
                }
            }

            ChangeCandidateKind::OperatorApproval {
                collection,
                owner,
                operator,
                ..
            } => {
                keys.token_contracts.push(collection);
                keys.collection_standards.push(collection);
                keys.operator_approvals.push(OperatorApprovalKey {
                    collection,
                    owner,
                    operator,
                });
            }
        }
    }

    retain_unique(&mut keys.token_contracts);
    retain_unique(&mut keys.collection_standards);
    retain_unique(&mut keys.erc20_balances);
    retain_unique(&mut keys.erc20_total_supplies);
    retain_unique(&mut keys.erc20_allowances);
    retain_unique(&mut keys.erc721_tokens);
    retain_unique(&mut keys.erc1155_balances);
    retain_unique(&mut keys.operator_approvals);

    keys
}
