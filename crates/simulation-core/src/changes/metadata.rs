//! Display metadata of the tokens that appear in the changes.

use std::collections::BTreeMap;

use alloy_sol_types::sol;

use super::{ApprovalChange, Asset, Derivation, Read, StateView, TokenMetadata};
use crate::ChainAddress;

sol! {
    function name() external view returns (string);
    function symbol() external view returns (string);
    function decimals() external view returns (uint8);
}

/// Reads metadata once per token. Metadata is optional in every standard, so
/// a failed read leaves the field empty.
pub(super) fn load<A: ChainAddress, V: StateView<A>>(
    derivation: &mut Derivation<'_, A, V>,
) -> Result<(), V::Error> {
    // Token address and whether it is fungible.
    let mut tokens = BTreeMap::new();
    for change in &derivation.changes.balances {
        match change.asset {
            Asset::Native => {}
            Asset::Erc20 { token } => *tokens.entry(token).or_default() = true,
            Asset::Erc721 { token, .. } | Asset::Erc1155 { token, .. } => {
                tokens.entry(token).or_default();
            }
        }
    }
    for change in &derivation.changes.approvals {
        match change {
            ApprovalChange::Erc20 { token, .. } => *tokens.entry(*token).or_default() = true,
            ApprovalChange::Erc721 { token, .. } | ApprovalChange::Operator { token, .. } => {
                tokens.entry(*token).or_default();
            }
        }
    }

    for (token, fungible) in tokens {
        let metadata = TokenMetadata {
            name: returned(derivation.read_latest(token, &nameCall {})?),
            symbol: returned(derivation.read_latest(token, &symbolCall {})?),
            decimals: if fungible {
                returned(derivation.read_latest(token, &decimalsCall {})?)
            } else {
                None
            },
        };
        derivation.changes.tokens.insert(token, metadata);
    }
    Ok(())
}

fn returned<T>(read: Read<T>) -> Option<T> {
    match read {
        Read::Returned(value) => Some(value),
        Read::NoCode | Read::Reverted | Read::Halted | Read::Malformed => None,
    }
}
