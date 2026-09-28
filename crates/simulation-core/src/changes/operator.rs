//! Operator approvals (`setApprovalForAll`), shared by ERC-721 and ERC-1155.

use std::collections::BTreeSet;

use alloy_sol_types::{SolEvent, sol};

use super::{ApprovalChange, Derivation, StateView, values};
use crate::ChainAddress;

sol! {
    event ApprovalForAll(address indexed owner, address indexed operator, bool approved);

    function isApprovedForAll(address owner, address operator) external view returns (bool);
}

pub(super) fn track<A: ChainAddress, V: StateView<A>>(
    derivation: &mut Derivation<'_, A, V>,
) -> Result<(), V::Error> {
    let approvals: BTreeSet<_> = derivation
        .trace
        .logs
        .iter()
        .filter_map(|log| {
            let event = ApprovalForAll::decode_log_data(&log.data).ok()?;
            Some((log.address, event.owner, event.operator))
        })
        .collect();

    for (token, owner, operator) in approvals {
        let value = derivation.read(token, &isApprovedForAllCall { owner, operator })?;
        let Some(value) = values(value, false) else {
            derivation.fail(token, "isApprovedForAll");
            continue;
        };
        if value.is_changed() {
            derivation.changes.approvals.push(ApprovalChange::Operator {
                token,
                owner: token.with_raw(owner),
                operator: token.with_raw(operator),
                before: value.before,
                after: value.after,
            });
        }
    }
    Ok(())
}
