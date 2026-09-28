//! ERC-1155 balances.

use std::collections::BTreeSet;

use alloy_primitives::U256;
use alloy_sol_types::{SolEvent, sol};

use super::{Asset, Derivation, StateView, is_holder, values};
use crate::ChainAddress;

sol! {
    event TransferSingle(
        address indexed operator,
        address indexed from,
        address indexed to,
        uint256 id,
        uint256 value
    );
    event TransferBatch(
        address indexed operator,
        address indexed from,
        address indexed to,
        uint256[] ids,
        uint256[] values
    );

    function balanceOf(address account, uint256 id) external view returns (uint256);
}

pub(super) fn track<A: ChainAddress, V: StateView<A>>(
    derivation: &mut Derivation<'_, A, V>,
) -> Result<(), V::Error> {
    let mut balances = BTreeSet::new();
    for log in &derivation.trace.logs {
        let token = log.address;
        if let Ok(event) = TransferSingle::decode_log_data(&log.data) {
            balances.extend([(token, event.from, event.id), (token, event.to, event.id)]);
        } else if let Ok(event) = TransferBatch::decode_log_data(&log.data) {
            for id in event.ids {
                balances.extend([(token, event.from, id), (token, event.to, id)]);
            }
        }
    }

    for (token, holder, id) in balances {
        if !is_holder(&holder) {
            continue;
        }
        let value = derivation.read(
            token,
            &balanceOfCall {
                account: holder,
                id,
            },
        )?;
        match values(value, U256::ZERO) {
            Some(value) => {
                derivation.balance(token.with_raw(holder), Asset::Erc1155 { token, id }, value)
            }
            None => derivation.fail(token, "balanceOf"),
        }
    }
    Ok(())
}
