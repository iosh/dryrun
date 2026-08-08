//! ERC-721 transaction state checks.

use std::collections::HashMap;

use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc721TokenKey, Erc721TokenState, Position, StandardCandidate,
    StandardCandidateKind, StandardStateValues, StatePhase, StateRequirement,
    change::legacy::{Change, PositionedChange},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Erc721TokenCursor {
    Absent,
    Present {
        owner: Address,
        approved_address: Option<Address>,
    },
}

impl Erc721TokenCursor {
    fn owner(self) -> Option<Address> {
        match self {
            Self::Absent => None,
            Self::Present { owner, .. } => Some(owner),
        }
    }
}

struct Erc721Replay {
    tokens: Vec<Erc721ReplayToken>,
    movements: Vec<PositionedChange>,
}

struct Erc721ReplayToken {
    key: Erc721TokenKey,
    cursor: Erc721TokenCursor,
    approval_position: Position,
}

pub(crate) fn check_erc721_changes(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedChange>, ContractStandardsError> {
    let replayed = replay_erc721_changes(candidates, before)?;
    let mut changes = replayed.movements;

    for replay_token in &replayed.tokens {
        let key = replay_token.key;
        let cursor = replay_token.cursor;
        let before_state = token_state(before, key, StatePhase::Before)?;
        let after_state = token_state(after, key, StatePhase::After)?;

        check_after_state(cursor, key, after_state)?;

        let approved_address_before = token_state_approval(before_state);
        let approved_address_after = token_state_approval(after_state);
        if approved_address_before == approved_address_after {
            continue;
        }

        changes.push(PositionedChange::new(
            replay_token.approval_position,
            Change::Erc721TokenApproval {
                contract_address: key.collection,
                token_id: key.token_id,
                approved_address_before,
                approved_address_after,
            },
        ));
    }

    Ok(changes)
}

fn replay_erc721_changes(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
) -> Result<Erc721Replay, ContractStandardsError> {
    let mut tokens = Vec::new();
    let mut token_indexes = HashMap::new();
    let mut movements = Vec::new();

    for candidate in candidates {
        match candidate.kind {
            StandardCandidateKind::Erc721Transfer {
                collection,
                from,
                to,
                token_id,
            } => {
                let key = Erc721TokenKey {
                    collection,
                    token_id,
                };
                let replay_token = token_entry(
                    &mut tokens,
                    &mut token_indexes,
                    before,
                    key,
                    candidate.position,
                )?;
                apply_movement(&mut replay_token.cursor, key, from, to)?;
                replay_token.approval_position = candidate.position;
                movements.push(PositionedChange::new(
                    candidate.position,
                    erc721_movement_change(collection, from, to, token_id),
                ));
            }

            StandardCandidateKind::Erc721Approval {
                collection,
                owner,
                approved_address,
                token_id,
            } => {
                let key = Erc721TokenKey {
                    collection,
                    token_id,
                };
                let replay_token = token_entry(
                    &mut tokens,
                    &mut token_indexes,
                    before,
                    key,
                    candidate.position,
                )?;
                apply_approval(&mut replay_token.cursor, key, owner, approved_address)?;
                replay_token.approval_position = candidate.position;
            }

            _ => {}
        }
    }

    Ok(Erc721Replay { tokens, movements })
}

fn erc721_movement_change(
    collection: Address,
    from: Address,
    to: Address,
    token_id: U256,
) -> Change {
    if from == Address::ZERO {
        Change::Erc721Mint {
            contract_address: collection,
            to,
            token_id,
        }
    } else if to == Address::ZERO {
        Change::Erc721Burn {
            contract_address: collection,
            from,
            token_id,
        }
    } else {
        Change::Erc721Transfer {
            contract_address: collection,
            from,
            to,
            token_id,
        }
    }
}

fn check_after_state(
    cursor: Erc721TokenCursor,
    key: Erc721TokenKey,
    after: Erc721TokenState,
) -> Result<(), ContractStandardsError> {
    match (cursor, after) {
        (Erc721TokenCursor::Absent, Erc721TokenState::OwnerOfReverted) => Ok(()),

        (
            Erc721TokenCursor::Present {
                owner,
                approved_address,
            },
            Erc721TokenState::Present {
                owner: after_owner,
                approved_address: after_approved_address,
            },
        ) if owner == after_owner => {
            if approved_address != after_approved_address {
                return Err(ContractStandardsError::Erc721ApprovalMismatch {
                    collection: key.collection,
                    token_id: key.token_id,
                    replayed_approved_address: approved_address,
                    after_approved_address,
                });
            }

            Ok(())
        }

        (cursor, after) => Err(ContractStandardsError::Erc721OwnerMismatch {
            collection: key.collection,
            token_id: key.token_id,
            replayed_owner: cursor.owner(),
            after_owner: token_state_owner(after),
        }),
    }
}

fn token_state_owner(state: Erc721TokenState) -> Option<Address> {
    match state {
        Erc721TokenState::Present { owner, .. } => Some(owner),
        Erc721TokenState::OwnerOfReverted => None,
    }
}

fn token_state_approval(state: Erc721TokenState) -> Option<Address> {
    match state {
        Erc721TokenState::Present {
            approved_address, ..
        } => approved_address,
        Erc721TokenState::OwnerOfReverted => None,
    }
}

fn token_entry<'a>(
    tokens: &'a mut Vec<Erc721ReplayToken>,
    token_indexes: &mut HashMap<Erc721TokenKey, usize>,
    before: &StandardStateValues,
    key: Erc721TokenKey,
    position: Position,
) -> Result<&'a mut Erc721ReplayToken, ContractStandardsError> {
    if let Some(&index) = token_indexes.get(&key) {
        return Ok(&mut tokens[index]);
    }

    let cursor = match token_state(before, key, StatePhase::Before)? {
        Erc721TokenState::Present {
            owner,
            approved_address,
        } => Erc721TokenCursor::Present {
            owner,
            approved_address,
        },

        // The first candidate is applied immediately. Only a mint can
        // turn this getter outcome into a valid absent-token path.
        Erc721TokenState::OwnerOfReverted => Erc721TokenCursor::Absent,
    };

    let index = tokens.len();
    tokens.push(Erc721ReplayToken {
        key,
        cursor,
        approval_position: position,
    });
    token_indexes.insert(key, index);

    Ok(&mut tokens[index])
}

fn apply_movement(
    cursor: &mut Erc721TokenCursor,
    key: Erc721TokenKey,
    from: Address,
    to: Address,
) -> Result<(), ContractStandardsError> {
    let current_owner = cursor.owner();

    match (from == Address::ZERO, to == Address::ZERO, *cursor) {
        (true, false, Erc721TokenCursor::Absent) => {
            *cursor = Erc721TokenCursor::Present {
                owner: to,
                approved_address: None,
            };
            Ok(())
        }

        (
            false,
            true,
            Erc721TokenCursor::Present {
                owner,
                approved_address: _,
            },
        ) if owner == from => {
            *cursor = Erc721TokenCursor::Absent;
            Ok(())
        }

        (
            false,
            false,
            Erc721TokenCursor::Present {
                owner,
                approved_address: _,
            },
        ) if owner == from => {
            *cursor = Erc721TokenCursor::Present {
                owner: to,
                approved_address: None,
            };
            Ok(())
        }

        _ => Err(ContractStandardsError::Erc721MovementInvalid {
            collection: key.collection,
            token_id: key.token_id,
            from,
            to,
            current_owner,
        }),
    }
}

fn apply_approval(
    cursor: &mut Erc721TokenCursor,
    key: Erc721TokenKey,
    event_owner: Address,
    approved_address: Option<Address>,
) -> Result<(), ContractStandardsError> {
    let current_owner = cursor.owner();

    match *cursor {
        Erc721TokenCursor::Present { owner, .. } if owner == event_owner => {
            *cursor = Erc721TokenCursor::Present {
                owner,
                approved_address,
            };
            Ok(())
        }

        _ => Err(ContractStandardsError::Erc721ApprovalInvalid {
            collection: key.collection,
            token_id: key.token_id,
            event_owner,
            current_owner,
        }),
    }
}

fn token_state(
    values: &StandardStateValues,
    key: Erc721TokenKey,
    phase: StatePhase,
) -> Result<Erc721TokenState, ContractStandardsError> {
    values
        .erc721_tokens
        .get(&key)
        .copied()
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc721Token(key),
            phase,
        })
}
