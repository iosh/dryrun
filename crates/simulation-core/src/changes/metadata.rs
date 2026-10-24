//! Display metadata of the tokens that appear in the changes.

use std::collections::BTreeMap;

use alloy_sol_types::{SolCall, sol};

use super::{ApprovalChange, Asset, Derivation, Read, StateView, TokenMetadata};
use crate::{ChainAddress, CodedError, ErrorCode};

sol! {
    function name() external view returns (string);
    function symbol() external view returns (string);
    function decimals() external view returns (uint8);
}

/// Reads metadata once per token. Contract failures leave a field empty;
/// resource limits stop all further metadata calls and preserve changes.
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

    let mut exhausted = false;
    for (token, fungible) in tokens {
        let metadata = TokenMetadata {
            name: read(derivation, token, &nameCall {}, &mut exhausted)?,
            symbol: read(derivation, token, &symbolCall {}, &mut exhausted)?,
            decimals: if fungible {
                read(derivation, token, &decimalsCall {}, &mut exhausted)?
            } else {
                None
            },
        };
        derivation.changes.tokens.insert(token, metadata);
    }
    Ok(())
}

fn read<A: ChainAddress, V: StateView<A>, C: SolCall>(
    derivation: &Derivation<'_, A, V>,
    token: A,
    call: &C,
    exhausted: &mut bool,
) -> Result<Option<C::Return>, V::Error> {
    if *exhausted {
        return Ok(None);
    }
    match derivation.read_latest(token, call) {
        Ok(Read::Returned(value)) => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) if error.code() == ErrorCode::LimitExceeded => {
            *exhausted = true;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
