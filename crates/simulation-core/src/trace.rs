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
    /// Accounts whose balance, nonce, code or storage changed.
    pub accounts: BTreeMap<A, AccountDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame<A> {
    /// Index of the calling frame in [`ExecutionTrace::calls`].
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
    pub nonce: Diff<u64>,
    pub code_hash: Diff<B256>,
    /// EIP-7702 delegation target.
    pub delegation: Diff<Option<Address>>,
    /// Slots whose value changed.
    pub storage: BTreeMap<B256, Diff<B256>>,
}

impl AccountDiff {
    /// Whether the account ran its own code before and after the execution.
    /// Delegated accounts are not contracts.
    pub fn is_contract(&self) -> Diff<bool> {
        let is_contract = |code_hash: B256, delegation: Option<Address>| {
            !code_hash.is_zero() && code_hash != KECCAK256_EMPTY && delegation.is_none()
        };
        Diff {
            before: is_contract(self.code_hash.before, self.delegation.before),
            after: is_contract(self.code_hash.after, self.delegation.after),
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
