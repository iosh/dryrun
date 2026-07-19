//! ERC-721 transaction state checks.

use std::collections::{HashMap, hash_map::Entry};

use alloy_primitives::Address;

use super::{
    candidate::{ChangeCandidate, ChangeCandidateKind},
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

pub(crate) fn check_erc721_changes(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    let replayed = replay_erc721_changes(candidates, before)?;

    for &key in &keys.erc721_tokens {
        let cursor =
            replayed
                .get(&key)
                .copied()
                .ok_or(TransactionChangesError::Erc721CandidateMissing {
                    collection: key.collection,
                    token_id: key.token_id,
                })?;
        let after_state = token_state(after, key, "after")?;

        check_after_state(cursor, key, after_state)?;
    }

    Ok(())
}

fn replay_erc721_changes(
    candidates: &[ChangeCandidate],
    before: &TokenStateValues,
) -> Result<HashMap<Erc721TokenKey, Erc721TokenCursor>, TransactionChangesError> {
    let mut cursors = HashMap::new();

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
            }

            _ => {}
        }
    }

    Ok(cursors)
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
