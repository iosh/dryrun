mod accounts;
mod governance;
mod pos;

use super::Address;
use crate::{
    Error,
    state::{StateSource, new_state},
    tracer::Tracer,
    view::View,
};
use alloy::primitives::{B256, U256};
use cfx_executor::{machine::Machine, state::State};
use cfx_vm_types::{Env, Spec};
use serde::Serialize;
use simulation_core::{ChangeSet, ExecutionTrace, FeePayment, derive_changes};
use std::{collections::BTreeSet, sync::Arc};
use tokio::runtime::Handle;

#[derive(Debug, Serialize)]
pub struct Changes {
    #[serde(flatten)]
    pub common: ChangeSet<Address>,
    pub protocol: Vec<ProtocolChange>,
}

/// Protocol state differences; amounts are read from state, not reconstructed from calls.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProtocolChange {
    StakingBalance {
        account: Address,
        before: U256,
        after: U256,
    },
    AccumulatedInterestReturn {
        account: Address,
        before: U256,
        after: U256,
    },
    CollateralForStorage {
        account: Address,
        before: U256,
        after: U256,
    },
    DepositList {
        account: Address,
        before: Vec<primitives::DepositInfo>,
        after: Vec<primitives::DepositInfo>,
    },
    VoteStakeList {
        account: Address,
        before: Vec<primitives::VoteStakeInfo>,
        after: Vec<primitives::VoteStakeInfo>,
    },
    PosIdentifier {
        account: Address,
        before: Option<B256>,
        after: Option<B256>,
    },
    PosStake {
        identifier: B256,
        before: PosStake,
        after: PosStake,
    },
    GovernanceVotes {
        voter: Address,
        before: Vec<GovernanceVote>,
        after: Vec<GovernanceVote>,
    },
    Admin {
        contract: Address,
        before: Option<Address>,
        after: Option<Address>,
    },
    GasSponsor {
        contract: Address,
        before: GasSponsor,
        after: GasSponsor,
    },
    StorageSponsor {
        contract: Address,
        before: StorageSponsor,
        after: StorageSponsor,
    },
    /// A null user denotes the public whitelist entry.
    SponsorWhitelist {
        contract: Address,
        user: Option<Address>,
        before: bool,
        after: bool,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GasSponsor {
    pub sponsor: Option<Address>,
    /// Actual pool balance, including any gas payment made by this transaction.
    pub balance: U256,
    pub gas_bound: U256,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSponsor {
    pub sponsor: Option<Address>,
    pub balance: U256,
    pub storage_points: Option<StoragePoints>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct StoragePoints {
    pub unused: U256,
    pub used: U256,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PosStake {
    pub account: Option<Address>,
    #[serde(with = "alloy_serde::quantity")]
    pub registered: u64,
    #[serde(with = "alloy_serde::quantity")]
    pub unlocked: u64,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GovernanceVote {
    #[serde(with = "alloy_serde::quantity")]
    pub index: u16,
    pub votes: [U256; 3],
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive(
    source: Arc<StateSource>,
    runtime: Handle,
    machine: &Machine,
    env: &Env,
    spec: &Spec,
    after: State,
    tracer: Tracer<'_, Address>,
    logs: Vec<primitives::LogEntry>,
    payment: &FeePayment<Address>,
) -> Result<Changes, Error> {
    let before = new_state(Arc::clone(&source), runtime)?;
    let trace = tracer.into_trace(&before, &after, logs)?;
    let mut protocol = accounts::derive(&before, &after, &trace, source.network)?;
    protocol.extend(pos::derive(&trace, &before, &after, source.network)?);
    let before = View::new(before, machine, env, spec, &source.budget);
    let after = View::new(after, machine, env, spec, &source.budget);
    protocol.extend(governance::derive(&trace, &before, &after, source.network)?);
    let common = derive_changes(&trace, payment, &before, &after)?;
    Ok(Changes { common, protocol })
}

/// Invocation candidates are needed only when the contract's storage changed.
fn contract_callers(
    trace: &ExecutionTrace<Address>,
    contract: Address,
) -> Result<BTreeSet<Address>, Error> {
    if !trace
        .accounts
        .get(&contract)
        .is_some_and(|account| !account.storage.is_empty())
    {
        return Ok(BTreeSet::new());
    }
    let callers: BTreeSet<_> = trace
        .calls
        .iter()
        .filter(|call| call.to == contract)
        .map(|call| call.from)
        .collect();
    if callers.is_empty() {
        return Err(Error::Internal(
            "protocol storage changed without an invoking frame",
        ));
    }
    Ok(callers)
}
