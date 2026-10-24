use std::collections::BTreeMap;

use alloy_primitives::{Address, B256, Bytes, KECCAK256_EMPTY, LogData, U256};

/// Facts produced by executing one transaction. Change derivation consumes
/// only this trace and state reads before and after the execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionTrace<A> {
    /// Frames that ran code, in the order they were entered. Calls to accounts
    /// without code and to precompiles are omitted.
    pub calls: Vec<CallFrame<A>>,
    /// Logs retained by the finished execution.
    pub logs: Vec<Log<A>>,
    /// Accounts touched by the execution, including unchanged ones: an
    /// unchanged balance can still hide a transfer that cancels out the fee.
    pub accounts: BTreeMap<A, AccountDiff>,
}

impl<A> ExecutionTrace<A> {
    /// Calls that succeeded and whose ancestors all succeeded.
    pub fn committed_calls(&self) -> impl Iterator<Item = &CallFrame<A>> {
        let mut committed = Vec::with_capacity(self.calls.len());
        self.calls.iter().filter(move |call| {
            let retained = call.success && call.parent.is_none_or(|parent| committed[parent]);
            committed.push(retained);
            retained
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame<A> {
    /// Index of the calling frame in [`ExecutionTrace::calls`].
    /// A parent is recorded before its children.
    pub parent: Option<usize>,
    pub scheme: CallScheme,
    /// `msg.sender` of the frame.
    pub from: A,
    /// `address(this)` of the frame, i.e. the account whose storage it uses.
    pub to: A,
    /// The account whose code runs; differs from `to` for `DELEGATECALL`,
    /// `CALLCODE` and delegated (EIP-7702) accounts.
    pub code_address: A,
    pub value: U256,
    pub input: Bytes,
    /// Whether the frame itself returned successfully. A successful frame is
    /// still discarded when an ancestor fails.
    pub success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallScheme {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
    Create,
    Create2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log<A> {
    pub address: A,
    pub data: LogData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDiff {
    pub balance: Diff<U256>,
    pub nonce: Diff<U256>,
    pub code_hash: Diff<B256>,
    /// EIP-7702 delegation target.
    pub delegation: Diff<Option<Address>>,
    /// Slots whose value changed.
    pub storage: BTreeMap<Bytes, Diff<B256>>,
}

impl AccountDiff {
    /// Whether the account has code on each side, including a delegation
    /// designation. Delegated code is executed by the state view.
    pub(crate) fn has_code(&self) -> Diff<bool> {
        let has_code = |hash: B256| !hash.is_zero() && hash != KECCAK256_EMPTY;
        Diff {
            before: has_code(self.code_hash.before),
            after: has_code(self.code_hash.after),
        }
    }

    /// Whether the account has non-delegated contract code on each side.
    pub fn is_contract(&self) -> Diff<bool> {
        let code = self.has_code();
        Diff {
            before: code.before && self.delegation.before.is_none(),
            after: code.after && self.delegation.after.is_none(),
        }
    }
}

/// A value before and after the execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Diff<T> {
    pub before: T,
    pub after: T,
}

impl<T: PartialEq> Diff<T> {
    pub fn is_changed(&self) -> bool {
        self.before != self.after
    }
}
