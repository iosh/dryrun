use std::collections::BTreeMap;

use super::InvolvedContract;
use crate::{ChainAddress, ExecutionTrace};

/// Contracts that ran code in the transaction or whose code or storage changed.
pub(super) fn involved<A: ChainAddress>(trace: &ExecutionTrace<A>) -> Vec<InvolvedContract<A>> {
    let mut contracts = BTreeMap::new();
    for call in &trace.calls {
        entry(&mut contracts, call.to).called = true;
        entry(&mut contracts, call.code_address).called = true;
    }
    for (address, account) in &trace.accounts {
        let is_contract = account.is_contract();
        let created = !is_contract.before && is_contract.after;
        let destroyed = is_contract.before && !is_contract.after;
        let storage_modified = !account.storage.is_empty();
        if created || destroyed || storage_modified {
            let contract = entry(&mut contracts, *address);
            contract.created = created;
            contract.destroyed = destroyed;
            contract.storage_modified = storage_modified;
        }
    }
    contracts.into_values().collect()
}

fn entry<A: ChainAddress>(
    contracts: &mut BTreeMap<A, InvolvedContract<A>>,
    address: A,
) -> &mut InvolvedContract<A> {
    contracts.entry(address).or_insert(InvolvedContract {
        address,
        called: false,
        created: false,
        destroyed: false,
        storage_modified: false,
    })
}
