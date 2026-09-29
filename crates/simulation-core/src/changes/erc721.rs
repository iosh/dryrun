//! ERC-721 ownership and single-token approvals.

use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use alloy_sol_types::{SolEvent, sol};

use super::{ApprovalChange, Asset, Derivation, Read, StateView};
use crate::{ChainAddress, Diff};

sol! {
    event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
    event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId);

    function ownerOf(uint256 tokenId) external view returns (address);
    function getApproved(uint256 tokenId) external view returns (address);
}

pub(super) fn track<A: ChainAddress, V: StateView<A>>(
    derivation: &mut Derivation<'_, A, V>,
) -> Result<(), V::Error> {
    // A transfer also clears the token's approval, so every token id that
    // appears in either event is read for both owner and approval. Each id
    // records whether the token may be missing before the execution (it was
    // minted) and after it (it was burned).
    let mut ids = BTreeMap::new();
    for log in &derivation.trace.logs {
        if let Ok(event) = Transfer::decode_log_data(&log.data) {
            let missing = ids.entry((log.address, event.tokenId)).or_insert(Diff {
                before: false,
                after: false,
            });
            missing.before |= event.from.is_zero();
            missing.after |= event.to.is_zero();
        } else if let Ok(event) = Approval::decode_log_data(&log.data) {
            ids.entry((log.address, event.tokenId)).or_insert(Diff {
                before: false,
                after: false,
            });
        }
    }

    for ((token, id), missing) in ids {
        let owner = derivation.read(token, &ownerOfCall { tokenId: id })?;
        match addresses(owner, missing) {
            Some(owner) if owner.is_changed() => {
                let asset = Asset::Erc721 { token, id };
                if let Some(previous) = owner.before {
                    derivation.balance(token.with_raw(previous), asset, ownership(true, false));
                }
                if let Some(next) = owner.after {
                    derivation.balance(token.with_raw(next), asset, ownership(false, true));
                }
            }
            Some(_) => {}
            None => derivation.fail(token, "ownerOf"),
        }

        let approved = derivation.read(token, &getApprovedCall { tokenId: id })?;
        match addresses(approved, missing) {
            Some(approved) if approved.is_changed() => {
                derivation.changes.approvals.push(ApprovalChange::Erc721 {
                    token,
                    id,
                    before: approved.before.map(|raw| token.with_raw(raw)),
                    after: approved.after.map(|raw| token.with_raw(raw)),
                });
            }
            Some(_) => {}
            None => derivation.fail(token, "getApproved"),
        }
    }
    Ok(())
}

/// The accounts read on both sides, where missing code or the zero address
/// means no account. ERC-721 getters revert for tokens that do not exist, so
/// a revert also means no account on a side where the token may be missing.
/// `None` if a call failed otherwise or returned malformed data.
fn addresses(value: Diff<Read<Address>>, missing: Diff<bool>) -> Option<Diff<Option<Address>>> {
    let account = |read: Read<Address>, missing: bool| match read {
        Read::NoCode => Some(None),
        Read::Reverted if missing => Some(None),
        Read::Returned(address) => Some((!address.is_zero()).then_some(address)),
        Read::Reverted | Read::Halted | Read::Malformed => None,
    };
    Some(Diff {
        before: account(value.before, missing.before)?,
        after: account(value.after, missing.after)?,
    })
}

/// Ownership of one token as a balance of zero or one.
fn ownership(before: bool, after: bool) -> Diff<U256> {
    Diff {
        before: U256::from(before),
        after: U256::from(after),
    }
}
