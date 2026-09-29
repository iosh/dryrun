//! Changes derived from an [`ExecutionTrace`] and state reads.
//!
//! Values always come from the state before and after the execution. Logs and
//! call frames only tell which balances and approvals to read. Each token
//! standard lives in its own module and follows the same steps: discover
//! candidates from the trace, then read them on both sides.

mod contracts;
mod erc1155;
mod erc20;
mod erc721;
mod metadata;
mod native;
mod operator;

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolCall;
use serde::Serialize;

use crate::{ChainAddress, Diff, ExecutionTrace};

/// Read-only access to the state on one side of the execution.
pub trait StateView<A> {
    type Error;

    /// Runs a read-only call and discards its writes.
    fn call(&self, contract: A, input: Bytes) -> Result<CallResult, Self::Error>;
}

/// How a read-only call ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult {
    Success(Bytes),
    Revert,
    /// The call failed without reverting, e.g. it ran out of gas.
    Halt,
}

/// The transaction fee as settled by the VM. It is reported separately and
/// removed from the native balance changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeePayment<A> {
    pub payer: A,
    pub amount: U256,
    pub beneficiary: A,
    pub reward: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet<A> {
    pub balances: Vec<BalanceChange<A>>,
    pub approvals: Vec<ApprovalChange<A>>,
    pub delegations: Vec<DelegationChange<A>>,
    /// Metadata of every token that appears in `balances` or `approvals`.
    pub tokens: BTreeMap<A, TokenMetadata>,
    pub contracts: Vec<InvolvedContract<A>>,
    /// Token contracts whose state could not be read with their standard ABI.
    /// Their changes are missing from `balances` and `approvals`.
    pub token_read_failures: Vec<TokenReadFailure<A>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceChange<A> {
    pub holder: A,
    pub asset: Asset<A>,
    pub before: U256,
    pub after: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Asset<A> {
    Native,
    Erc20 { token: A },
    Erc721 { token: A, id: U256 },
    Erc1155 { token: A, id: U256 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ApprovalChange<A> {
    Erc20 {
        token: A,
        owner: A,
        spender: A,
        before: U256,
        after: U256,
    },
    /// The address approved for one ERC-721 token.
    Erc721 {
        token: A,
        id: U256,
        before: Option<A>,
        after: Option<A>,
    },
    /// `setApprovalForAll`, shared by ERC-721 and ERC-1155.
    Operator {
        token: A,
        owner: A,
        operator: A,
        before: bool,
        after: bool,
    },
}

/// EIP-7702 delegation of an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DelegationChange<A> {
    pub account: A,
    pub before: Option<A>,
    pub after: Option<A>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TokenMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
}

/// A contract the transaction called or whose state it changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvolvedContract<A> {
    pub address: A,
    pub called: bool,
    pub created: bool,
    pub destroyed: bool,
    pub storage_modified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct TokenReadFailure<A> {
    pub token: A,
    /// The standard function that failed or returned malformed data.
    pub function: &'static str,
}

/// Derives the changes of a successful execution.
///
/// `before` and `after` read the state before and after the execution.
/// Errors of the views propagate; a token that does not follow its standard
/// is reported in [`ChangeSet::token_read_failures`] instead.
pub fn derive_changes<A: ChainAddress, V: StateView<A>>(
    trace: &ExecutionTrace<A>,
    fee: &FeePayment<A>,
    before: &V,
    after: &V,
) -> Result<ChangeSet<A>, V::Error> {
    let mut derivation = Derivation {
        trace,
        before,
        after,
        changes: ChangeSet {
            balances: native::balances(trace, fee),
            approvals: Vec::new(),
            delegations: native::delegations(trace),
            tokens: BTreeMap::new(),
            contracts: contracts::involved(trace),
            token_read_failures: Vec::new(),
        },
        failures: BTreeSet::new(),
    };
    erc20::track(&mut derivation)?;
    erc721::track(&mut derivation)?;
    erc1155::track(&mut derivation)?;
    operator::track(&mut derivation)?;
    metadata::load(&mut derivation)?;

    let mut changes = derivation.changes;
    changes.token_read_failures = derivation.failures.into_iter().collect();
    Ok(changes)
}

/// State shared by the token trackers of one derivation.
struct Derivation<'a, A, V> {
    trace: &'a ExecutionTrace<A>,
    before: &'a V,
    after: &'a V,
    changes: ChangeSet<A>,
    failures: BTreeSet<TokenReadFailure<A>>,
}

/// The result of a read-only call on one side of the execution.
enum Read<T> {
    /// The contract has no code on this side, so the call was not made.
    NoCode,
    Reverted,
    Halted,
    Malformed,
    Returned(T),
}

impl<A: ChainAddress, V: StateView<A>> Derivation<'_, A, V> {
    /// Calls `call` on `contract` before and after the execution.
    fn read<C: SolCall>(&self, contract: A, call: &C) -> Result<Diff<Read<C::Return>>, V::Error> {
        let is_contract = self.is_contract(contract);
        Ok(Diff {
            before: read(self.before, is_contract.before, contract, call)?,
            after: read(self.after, is_contract.after, contract, call)?,
        })
    }

    /// Calls `call` on `contract` after the execution, or before it if the
    /// execution destroyed the contract.
    fn read_latest<C: SolCall>(&self, contract: A, call: &C) -> Result<Read<C::Return>, V::Error> {
        let is_contract = self.is_contract(contract);
        if is_contract.before && !is_contract.after {
            read(self.before, true, contract, call)
        } else {
            read(self.after, is_contract.after, contract, call)
        }
    }

    fn is_contract(&self, contract: A) -> Diff<bool> {
        // Accounts missing from the trace kept their code.
        self.trace.accounts.get(&contract).map_or(
            Diff {
                before: true,
                after: true,
            },
            |account| account.is_contract(),
        )
    }

    fn fail(&mut self, token: A, function: &'static str) {
        self.failures.insert(TokenReadFailure { token, function });
    }

    /// Records a token balance whose values were read on both sides.
    fn balance(&mut self, holder: A, asset: Asset<A>, value: Diff<U256>) {
        if value.is_changed() {
            self.changes.balances.push(BalanceChange {
                holder,
                asset,
                before: value.before,
                after: value.after,
            });
        }
    }
}

fn read<A: Copy, V: StateView<A>, C: SolCall>(
    view: &V,
    is_contract: bool,
    contract: A,
    call: &C,
) -> Result<Read<C::Return>, V::Error> {
    if !is_contract {
        return Ok(Read::NoCode);
    }
    Ok(match view.call(contract, call.abi_encode().into())? {
        CallResult::Success(output) => {
            C::abi_decode_returns_validate(&output).map_or(Read::Malformed, Read::Returned)
        }
        CallResult::Revert => Read::Reverted,
        CallResult::Halt => Read::Halted,
    })
}

/// Values read on both sides, where a side without code holds `empty`.
/// `None` if a call failed or returned malformed data.
fn values<T: Clone>(read: Diff<Read<T>>, empty: T) -> Option<Diff<T>> {
    let value = |read| match read {
        Read::NoCode => Some(empty.clone()),
        Read::Returned(value) => Some(value),
        Read::Reverted | Read::Halted | Read::Malformed => None,
    };
    Some(Diff {
        before: value(read.before)?,
        after: value(read.after)?,
    })
}

/// Topic-decoded holders of token logs include the zero address for mints and
/// burns; it holds nothing worth reading.
fn is_holder(raw: &Address) -> bool {
    !raw.is_zero()
}
