//! ERC-20 balances and allowances, including wrapped native tokens whose
//! deposits and withdrawals emit `Deposit` and `Withdrawal` instead of `Transfer`.

use std::collections::BTreeSet;

use alloy_primitives::U256;
use alloy_sol_types::{SolCall, SolEvent, sol};

use super::{ApprovalChange, Asset, Derivation, StateView, is_holder, values};
use crate::ChainAddress;

sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    event Deposit(address indexed dst, uint256 wad);
    event Withdrawal(address indexed src, uint256 wad);

    function transferFrom(address from, address to, uint256 value) external returns (bool);
    function balanceOf(address owner) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
}

pub(super) fn track<A: ChainAddress, V: StateView<A>>(
    derivation: &mut Derivation<'_, A, V>,
) -> Result<(), V::Error> {
    let trace = derivation.trace;
    // Contracts that emitted `Transfer` or `Approval`. Other contracts also
    // emit `Deposit` and `Withdrawal`, so those only count if `balanceOf` works.
    let mut tokens = BTreeSet::new();
    let mut holders = BTreeSet::new();
    let mut allowances = BTreeSet::new();

    for log in &trace.logs {
        let token = log.address;
        if let Ok(event) = Transfer::decode_log_data(&log.data) {
            tokens.insert(token);
            holders.extend([(token, event.from), (token, event.to)]);
        } else if let Ok(event) = Approval::decode_log_data(&log.data) {
            tokens.insert(token);
            allowances.insert((token, event.owner, event.spender));
        } else if let Ok(event) = Deposit::decode_log_data(&log.data) {
            holders.insert((token, event.dst));
        } else if let Ok(event) = Withdrawal::decode_log_data(&log.data) {
            holders.insert((token, event.src));
        }
    }
    // `transferFrom` spends an allowance without a required `Approval` event.
    // Its selector is shared with ERC-721, so only known ERC-20 tokens count.
    for call in &trace.calls {
        if tokens.contains(&call.to)
            && let Ok(transfer) = transferFromCall::abi_decode(&call.input)
        {
            allowances.insert((call.to, transfer.from, call.from.raw()));
        }
    }

    for (token, holder) in holders {
        if !is_holder(&holder) {
            continue;
        }
        let value = derivation.read(token, &balanceOfCall { owner: holder })?;
        match values(value, U256::ZERO) {
            Some(value) => {
                derivation.balance(token.with_raw(holder), Asset::Erc20 { token }, value)
            }
            None if tokens.contains(&token) => derivation.fail(token, "balanceOf"),
            None => {}
        }
    }

    for (token, owner, spender) in allowances {
        let value = derivation.read(token, &allowanceCall { owner, spender })?;
        let Some(value) = values(value, U256::ZERO) else {
            derivation.fail(token, "allowance");
            continue;
        };
        if value.is_changed() {
            derivation.changes.approvals.push(ApprovalChange::Erc20 {
                token,
                owner: token.with_raw(owner),
                spender: token.with_raw(spender),
                before: value.before,
                after: value.after,
            });
        }
    }
    Ok(())
}
