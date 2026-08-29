use std::{collections::BTreeMap, error::Error as StdError, sync::Arc};

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

    #[error("change resolver could not produce complete changes")]
    Resolver {
        resolver: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
}

impl EvmChangeResolutionError {
    pub fn resolver(resolver: &'static str, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Resolver {
            resolver,
            source: Box::new(source),
        }
    }
}

#[derive(Debug, Default)]
pub struct EvmChangeSetBuilder {
    items: BTreeMap<EvmChangeKey, EvmStateChange>,
}

impl EvmChangeSetBuilder {
    pub fn new() -> Self {
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

    pub fn native_balance(
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

    pub fn account_delegation(
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

    pub fn finish(self) -> EvmChangeSet {
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

pub trait EvmChangeResolver: Send + Sync + 'static {
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError>;

    fn combine<R>(self, other: R) -> CombinedEvmChangeResolver<Self, R>
    where
        Self: Sized,
        R: EvmChangeResolver,
    {
        CombinedEvmChangeResolver::new(self, other)
    }
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
pub struct EvmNativeBalanceResolver {
    currency: EvmNativeCurrency,
}

impl EvmNativeBalanceResolver {
    pub fn new(currency: EvmNativeCurrency) -> Self {
        Self { currency }
    }
}

impl EvmChangeResolver for EvmNativeBalanceResolver {
    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        views: &EvmStateViews,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        let mut builder = EvmChangeSetBuilder::new();
        for &account in execution.native_balance_accounts() {
            let before = views.before().read_account(account)?.balance();
            let after = views.after().read_account(account)?.balance();
            builder.native_balance(account, before, after, self.currency.clone())?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EvmAccountDelegationResolver;

impl EvmChangeResolver for EvmAccountDelegationResolver {
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
                    delegate: before.delegation(),
                    nonce: before.nonce(),
                },
                EvmAccountDelegation {
                    delegate: after.delegation(),
                    nonce: after.nonce(),
                },
            )?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Clone)]
pub struct StandardEvmChangeResolver {
    components: CombinedEvmChangeResolver<EvmNativeBalanceResolver, EvmAccountDelegationResolver>,
}

impl StandardEvmChangeResolver {
    pub fn new(currency: EvmNativeCurrency) -> Self {
        Self {
            components: CombinedEvmChangeResolver::new(
                EvmNativeBalanceResolver::new(currency),
                EvmAccountDelegationResolver,
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
pub struct CombinedEvmChangeResolver<A, B> {
    first: Arc<A>,
    second: Arc<B>,
}

impl<A, B> CombinedEvmChangeResolver<A, B> {
    pub fn new(first: A, second: B) -> Self {
        Self {
            first: Arc::new(first),
            second: Arc::new(second),
        }
    }

    pub(crate) fn from_shared(first: Arc<A>, second: B) -> Self {
        Self {
            first,
            second: Arc::new(second),
        }
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
