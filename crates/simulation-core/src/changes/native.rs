use super::{Asset, BalanceChange, DelegationChange, FeePayment};
use crate::{ChainAddress, ExecutionTrace};

/// Native balance changes, excluding the fee: the payer's charge is added back
/// and the beneficiary's reward is taken out.
pub(super) fn balances<A: ChainAddress>(
    trace: &ExecutionTrace<A>,
    fee: &FeePayment<A>,
) -> Vec<BalanceChange<A>> {
    trace
        .accounts
        .iter()
        .filter_map(|(address, account)| {
            let mut after = account.balance.after;
            if Some(*address) == fee.payer {
                after += fee.amount;
            }
            if *address == fee.beneficiary {
                after -= fee.reward;
            }
            (after != account.balance.before).then_some(BalanceChange {
                holder: *address,
                asset: Asset::Native,
                before: account.balance.before,
                after,
            })
        })
        .collect()
}

pub(super) fn delegations<A: ChainAddress>(trace: &ExecutionTrace<A>) -> Vec<DelegationChange<A>> {
    trace
        .accounts
        .iter()
        .filter(|(_, account)| account.delegation.is_changed())
        .map(|(address, account)| DelegationChange {
            account: *address,
            before: account.delegation.before.map(|raw| address.with_raw(raw)),
            after: account.delegation.after.map(|raw| address.with_raw(raw)),
        })
        .collect()
}
