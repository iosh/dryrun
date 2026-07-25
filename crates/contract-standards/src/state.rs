use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
};

use alloy_primitives::{Address, B256, U256};

use crate::candidate::{StandardCandidate, StandardCandidateKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Erc20BalanceKey {
    pub token: Address,
    pub account: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Erc20AllowanceKey {
    pub token: Address,
    pub owner: Address,
    pub spender: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Erc721TokenKey {
    pub collection: Address,
    pub token_id: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Erc1155BalanceKey {
    pub collection: Address,
    pub account: Address,
    pub token_id: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperatorApprovalKey {
    pub collection: Address,
    pub owner: Address,
    pub operator: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateRequirement {
    TokenContractCode(Address),
    CollectionStandards(Address),
    Erc20Balance(Erc20BalanceKey),
    Erc20TotalSupply(Address),
    Erc20Allowance(Erc20AllowanceKey),
    Erc721Token(Erc721TokenKey),
    Erc1155Balance(Erc1155BalanceKey),
    OperatorApproval(OperatorApprovalKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatePhase {
    Before,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateArithmeticOperation {
    Add,
    Subtract,
}

impl fmt::Display for StateRequirement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenContractCode(address) => {
                write!(formatter, "runtime code hash for token contract {address}")
            }
            Self::CollectionStandards(collection) => {
                write!(formatter, "collection standards for {collection}")
            }
            Self::Erc20Balance(key) => {
                write!(
                    formatter,
                    "ERC-20 balance for {} in token {}",
                    key.account, key.token
                )
            }
            Self::Erc20TotalSupply(token) => {
                write!(formatter, "ERC-20 total supply for token {token}")
            }
            Self::Erc20Allowance(key) => write!(
                formatter,
                "ERC-20 allowance for owner {} and spender {} in token {}",
                key.owner, key.spender, key.token
            ),
            Self::Erc721Token(key) => write!(
                formatter,
                "ERC-721 state for token {} in collection {}",
                key.token_id, key.collection
            ),
            Self::Erc1155Balance(key) => write!(
                formatter,
                "ERC-1155 balance for {} and token {} in collection {}",
                key.account, key.token_id, key.collection
            ),
            Self::OperatorApproval(key) => write!(
                formatter,
                "operator approval for owner {} and operator {} in collection {}",
                key.owner, key.operator, key.collection
            ),
        }
    }
}

impl fmt::Display for StatePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Before => "before",
            Self::After => "after",
        })
    }
}

impl fmt::Display for StateArithmeticOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Add => "addition",
            Self::Subtract => "subtraction",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateRequirements {
    pub token_contracts: Vec<Address>,
    pub collection_standards: Vec<Address>,
    pub erc20_balances: Vec<Erc20BalanceKey>,
    pub erc20_total_supplies: Vec<Address>,
    pub erc20_allowances: Vec<Erc20AllowanceKey>,
    pub erc721_tokens: Vec<Erc721TokenKey>,
    pub erc1155_balances: Vec<Erc1155BalanceKey>,
    pub operator_approvals: Vec<OperatorApprovalKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionStandards {
    pub supports_erc721: bool,
    pub supports_erc1155: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Erc721TokenState {
    Present {
        owner: Address,
        approved_address: Option<Address>,
    },
    OwnerOfReverted,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StandardStateValues {
    pub contract_code_hashes: HashMap<Address, B256>,
    pub collection_standards: HashMap<Address, CollectionStandards>,
    pub erc20_balances: HashMap<Erc20BalanceKey, U256>,
    pub erc20_total_supplies: HashMap<Address, U256>,
    pub erc20_allowances: HashMap<Erc20AllowanceKey, U256>,
    pub erc721_tokens: HashMap<Erc721TokenKey, Erc721TokenState>,
    pub erc1155_balances: HashMap<Erc1155BalanceKey, U256>,
    pub operator_approvals: HashMap<OperatorApprovalKey, bool>,
}

fn retain_unique<T>(values: &mut Vec<T>)
where
    T: Copy + Eq + Hash,
{
    let mut seen = HashSet::with_capacity(values.len());
    values.retain(|value| seen.insert(*value));
}

pub fn state_requirements(candidates: &[StandardCandidate]) -> StateRequirements {
    let mut keys = StateRequirements::default();

    for candidate in candidates {
        match candidate.kind {
            StandardCandidateKind::Erc20Movement {
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

            StandardCandidateKind::Erc20Allowance {
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

            StandardCandidateKind::Erc721Transfer {
                collection,
                token_id,
                ..
            }
            | StandardCandidateKind::Erc721Approval {
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

            StandardCandidateKind::Erc1155Transfer {
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

            StandardCandidateKind::OperatorApproval {
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
