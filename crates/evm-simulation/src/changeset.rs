use std::collections::BTreeMap;

use alloy::primitives::{Address, U256};
use thiserror::Error;

use crate::{
    execution::EvmTransactionExecution,
    state::{EvmStateReadError, EvmStateViews},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmNativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EvmChangeSet {
    items: Vec<EvmStateChange>,
}

impl EvmChangeSet {
    pub fn items(&self) -> &[EvmStateChange] {
        &self.items
    }

    pub fn into_items(self) -> Vec<EvmStateChange> {
        self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn merge(self, other: Self) -> Result<Self, EvmChangeResolutionError> {
        let mut builder = EvmChangeSetBuilder::new();
        for item in self.items.into_iter().chain(other.items) {
            builder.insert(item)?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmStateChange {
    NativeBalance(EvmNativeBalanceChange),
    AccountDelegation(EvmAccountDelegationChange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmNativeBalanceChange {
    pub account: Address,
    pub before: U256,
    pub after: U256,
    pub currency: EvmNativeCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmAccountDelegationChange {
    pub account: Address,
    pub before: EvmAccountDelegation,
    pub after: EvmAccountDelegation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvmAccountDelegation {
    pub delegate: Option<Address>,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum EvmChangeKey {
    NativeBalance(Address),
    AccountDelegation(Address),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmChangeResolutionError {
    #[error(transparent)]
    StateRead(#[from] EvmStateReadError),

    #[error("conflicting native balance values for {account}")]
    NativeBalanceConflict { account: Address },

    #[error("conflicting delegation values for account {account}")]
    AccountDelegationConflict { account: Address },
}

#[derive(Debug, Default)]
struct EvmChangeSetBuilder {
    items: BTreeMap<EvmChangeKey, EvmStateChange>,
}

impl EvmChangeSetBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn insert(&mut self, item: EvmStateChange) -> Result<(), EvmChangeResolutionError> {
        let key = item.key();
        if let Some(existing) = self.items.get(&key) {
            if existing == &item {
                return Ok(());
            }
            return Err(match key {
                EvmChangeKey::NativeBalance(account) => {
                    EvmChangeResolutionError::NativeBalanceConflict { account }
                }
                EvmChangeKey::AccountDelegation(account) => {
                    EvmChangeResolutionError::AccountDelegationConflict { account }
                }
            });
        }
        self.items.insert(key, item);
        Ok(())
    }

    fn native_balance(
        &mut self,
        account: Address,
        before: U256,
        after: U256,
        currency: EvmNativeCurrency,
    ) -> Result<(), EvmChangeResolutionError> {
        if before != after {
            self.insert(EvmStateChange::NativeBalance(EvmNativeBalanceChange {
                account,
                before,
                after,
                currency,
            }))?;
        }
        Ok(())
    }

    fn account_delegation(
        &mut self,
        account: Address,
        before: EvmAccountDelegation,
        after: EvmAccountDelegation,
    ) -> Result<(), EvmChangeResolutionError> {
        if before != after {
            self.insert(EvmStateChange::AccountDelegation(
                EvmAccountDelegationChange {
                    account,
                    before,
                    after,
                },
            ))?;
        }
        Ok(())
    }

    fn finish(self) -> EvmChangeSet {
        EvmChangeSet {
            items: self.items.into_values().collect(),
        }
    }
}

impl EvmStateChange {
    fn key(&self) -> EvmChangeKey {
        match self {
            Self::NativeBalance(change) => EvmChangeKey::NativeBalance(change.account),
            Self::AccountDelegation(change) => EvmChangeKey::AccountDelegation(change.account),
        }
    }
}

pub(crate) trait EvmChangeResolver {
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError>;
}

#[derive(Debug)]
pub enum EvmChanges {
    Complete(EvmChangeSet),
    Unavailable(EvmChangeResolutionError),
}

impl From<Result<EvmChangeSet, EvmChangeResolutionError>> for EvmChanges {
    fn from(result: Result<EvmChangeSet, EvmChangeResolutionError>) -> Self {
        match result {
            Ok(changes) => Self::Complete(changes),
            Err(error) => Self::Unavailable(error),
        }
    }
}

#[derive(Debug, Clone)]
struct NativeBalanceResolver {
    currency: EvmNativeCurrency,
}

impl EvmChangeResolver for NativeBalanceResolver {
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        let mut builder = EvmChangeSetBuilder::new();
        for &account in execution.native_balance_accounts() {
            let before = views.before().read_account(account)?.balance;
            let after = views.after().read_account(account)?.balance;
            builder.native_balance(account, before, after, self.currency.clone())?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct AccountDelegationResolver;

impl EvmChangeResolver for AccountDelegationResolver {
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        let mut builder = EvmChangeSetBuilder::new();
        for &account in execution.applied_authorization_accounts() {
            let before = views.before().read_account(account)?;
            let after = views.after().read_account(account)?;
            builder.account_delegation(
                account,
                EvmAccountDelegation {
                    delegate: before.delegation,
                    nonce: before.nonce,
                },
                EvmAccountDelegation {
                    delegate: after.delegation,
                    nonce: after.nonce,
                },
            )?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StandardEvmChangeResolver {
    components: CombinedEvmChangeResolver<NativeBalanceResolver, AccountDelegationResolver>,
}

impl StandardEvmChangeResolver {
    pub(crate) fn new(currency: EvmNativeCurrency) -> Self {
        Self {
            components: CombinedEvmChangeResolver::new(
                NativeBalanceResolver { currency },
                AccountDelegationResolver,
            ),
        }
    }
}

impl EvmChangeResolver for StandardEvmChangeResolver {
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        self.components.resolve(execution, views)
    }
}

#[derive(Debug, Clone)]
struct CombinedEvmChangeResolver<A, B> {
    first: A,
    second: B,
}

impl<A, B> CombinedEvmChangeResolver<A, B> {
    const fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<A, B> EvmChangeResolver for CombinedEvmChangeResolver<A, B>
where
    A: EvmChangeResolver,
    B: EvmChangeResolver,
{
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        let first = self.first.resolve(execution, views)?;
        let second = self.second.resolve(execution, views)?;
        first.merge(second)
    }
}
