//! ERC-721 transaction state checks.

use std::collections::{HashMap, hash_map::Entry};

use alloy_primitives::{Address, U256};

use crate::{Change, Erc721CollectionMetadata};

use super::{
    PositionedChange,
    candidate::{ChangeCandidate, ChangeCandidateKind, ObservationPosition},
    error::TransactionChangesError,
    token_state::{Erc721TokenKey, Erc721TokenState, TokenStateKeys, TokenStateValues},
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
    cursors: HashMap<Erc721TokenKey, Erc721TokenCursor>,
    approval_positions: HashMap<Erc721TokenKey, ObservationPosition>,
    movements: Vec<PositionedChange>,
}

pub(crate) fn check_erc721_changes(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, TransactionChangesError> {
    let replayed = replay_erc721_changes(candidates, before)?;
    let mut changes = replayed.movements;

    for &key in &keys.erc721_tokens {
        let cursor = replayed.cursors.get(&key).copied().ok_or(
            TransactionChangesError::Erc721CandidateMissing {
                collection: key.collection,
                token_id: key.token_id,
            },
        )?;
        let before_state = token_state(before, key, "before")?;
        let after_state = token_state(after, key, "after")?;

        check_after_state(cursor, key, after_state)?;

        let approved_address_before = token_state_approval(before_state);
        let approved_address_after = token_state_approval(after_state);
        if approved_address_before == approved_address_after {
            continue;
        }

        let position = replayed.approval_positions.get(&key).copied().ok_or(
            TransactionChangesError::Erc721CandidateMissing {
                collection: key.collection,
                token_id: key.token_id,
            },
        )?;
        changes.push(PositionedChange::new(
            position,
            Change::Erc721TokenApproval {
                contract_address: key.collection,
                token_id: key.token_id,
                approved_address_before,
                approved_address_after,
                metadata: Erc721CollectionMetadata::default(),
            },
        ));
    }

    Ok(changes)
}

fn replay_erc721_changes(
    candidates: &[ChangeCandidate],
    before: &TokenStateValues,
) -> Result<Erc721Replay, TransactionChangesError> {
    let mut cursors = HashMap::new();
    let mut approval_positions = HashMap::new();
    let mut movements = Vec::new();

    for candidate in candidates {
        match candidate.kind {
            ChangeCandidateKind::Erc721Transfer {
                collection,
                from,
                to,
                token_id,
            } => {
                let key = Erc721TokenKey {
                    collection,
                    token_id,
                };
                let cursor = token_cursor(&mut cursors, before, key)?;
                apply_movement(cursor, key, from, to)?;
                approval_positions.insert(key, candidate.position);
                movements.push(PositionedChange::new(
                    candidate.position,
                    erc721_movement_change(collection, from, to, token_id),
                ));
            }

            ChangeCandidateKind::Erc721Approval {
                collection,
                owner,
                approved_address,
                token_id,
            } => {
                let key = Erc721TokenKey {
                    collection,
                    token_id,
                };
                let cursor = token_cursor(&mut cursors, before, key)?;
                apply_approval(cursor, key, owner, approved_address)?;
                approval_positions.insert(key, candidate.position);
            }

            _ => {}
        }
    }

    Ok(Erc721Replay {
        cursors,
        approval_positions,
        movements,
    })
}

fn erc721_movement_change(
    collection: Address,
    from: Address,
    to: Address,
    token_id: U256,
) -> Change {
    let metadata = Erc721CollectionMetadata::default();

    if from == Address::ZERO {
        Change::Erc721Mint {
            contract_address: collection,
            to,
            token_id,
            metadata,
        }
    } else if to == Address::ZERO {
        Change::Erc721Burn {
            contract_address: collection,
            from,
            token_id,
            metadata,
        }
    } else {
        Change::Erc721Transfer {
            contract_address: collection,
            from,
            to,
            token_id,
            metadata,
        }
    }
}

fn check_after_state(
    cursor: Erc721TokenCursor,
    key: Erc721TokenKey,
    after: Erc721TokenState,
) -> Result<(), TransactionChangesError> {
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
                return Err(TransactionChangesError::Erc721ApprovalMismatch {
                    collection: key.collection,
                    token_id: key.token_id,
                    replayed_approved_address: approved_address,
                    after_approved_address,
                });
            }

            Ok(())
        }

        (cursor, after) => Err(TransactionChangesError::Erc721OwnerMismatch {
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

fn token_cursor<'a>(
    cursors: &'a mut HashMap<Erc721TokenKey, Erc721TokenCursor>,
    before: &TokenStateValues,
    key: Erc721TokenKey,
) -> Result<&'a mut Erc721TokenCursor, TransactionChangesError> {
    match cursors.entry(key) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => {
            let cursor = match token_state(before, key, "before")? {
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

            Ok(entry.insert(cursor))
        }
    }
}

fn apply_movement(
    cursor: &mut Erc721TokenCursor,
    key: Erc721TokenKey,
    from: Address,
    to: Address,
) -> Result<(), TransactionChangesError> {
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

        _ => Err(TransactionChangesError::Erc721MovementInvalid {
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
) -> Result<(), TransactionChangesError> {
    let current_owner = cursor.owner();

    match *cursor {
        Erc721TokenCursor::Present { owner, .. } if owner == event_owner => {
            *cursor = Erc721TokenCursor::Present {
                owner,
                approved_address,
            };
            Ok(())
        }

        _ => Err(TransactionChangesError::Erc721ApprovalInvalid {
            collection: key.collection,
            token_id: key.token_id,
            event_owner,
            current_owner,
        }),
    }
}

fn token_state(
    values: &TokenStateValues,
    key: Erc721TokenKey,
    state: &'static str,
) -> Result<Erc721TokenState, TransactionChangesError> {
    values.erc721_tokens.get(&key).copied().ok_or(
        TransactionChangesError::Erc721TokenStateMissing {
            collection: key.collection,
            token_id: key.token_id,
            state,
        },
    )
}
