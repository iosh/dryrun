use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use contract_standards::{MetadataKind, MetadataStore};
use contract_standards::{StandardChange, StandardEffect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangePosition {
    BeforeExecution,
    Execution(usize),
    Settlement,
}

/// Stores one authoritative value per occurrence and semantic object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedChanges<K, C> {
    entries: BTreeMap<(ChangePosition, K), C>,
}

impl<K, C> Default for OrderedChanges<K, C> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}
impl<K: Ord, C> OrderedChanges<K, C> {
    pub fn insert(&mut self, position: ChangePosition, object: K, change: C) {
        self.entries.insert((position, object), change);
    }
    pub fn merge(mut self, other: Self) -> Self {
        self.entries.extend(other.entries);
        self
    }
    pub fn items(&self) -> impl ExactSizeIterator<Item = &C> {
        self.entries.values()
    }
    pub fn into_items(self) -> Vec<C> {
        self.entries.into_values().collect()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (ChangePosition, &C)> {
        self.entries
            .iter()
            .map(|((position, _), change)| (*position, change))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct NativeTransfer<A = Address> {
    pub from: A,
    pub to: A,
    pub raw_amount: U256,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub currency: NativeCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct NativeBurn<A = Address> {
    pub contract_address: A,
    pub raw_amount: U256,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub currency: NativeCurrency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct AccountDelegation<A = Address> {
    pub delegate: Option<A>,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DelegationChange<A = Address> {
    pub account: A,
    pub before: AccountDelegation<A>,
    pub after: AccountDelegation<A>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct WrappedNativeChange<A = Address> {
    pub contract_address: A,
    pub account: A,
    pub raw_amount: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase"))]
pub enum AssetChange<A = Address> {
    NativeTransfer(NativeTransfer<A>),
    SelfDestructBurn(NativeBurn<A>),
    AccountDelegation(DelegationChange<A>),
    WrappedNativeDeposit(WrappedNativeChange<A>),
    WrappedNativeWithdrawal(WrappedNativeChange<A>),
    #[cfg_attr(feature = "serde", serde(untagged))]
    Standard(StandardChange<A>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetEffect<A = Address> {
    NativeTransfer { from: A, to: A },
    NativeBurn(A),
    Delegation(A),
    Standard(StandardEffect<A>),
}

impl<A: Clone> AssetChange<A> {
    pub fn try_map_addresses<B, E>(
        self,
        mut map: impl FnMut(A) -> Result<B, E>,
    ) -> Result<AssetChange<B>, E> {
        Ok(match self {
            Self::NativeTransfer(change) => AssetChange::NativeTransfer(NativeTransfer {
                from: map(change.from)?,
                to: map(change.to)?,
                raw_amount: change.raw_amount,
                currency: change.currency,
            }),
            Self::SelfDestructBurn(change) => AssetChange::SelfDestructBurn(NativeBurn {
                contract_address: map(change.contract_address)?,
                raw_amount: change.raw_amount,
                currency: change.currency,
            }),
            Self::AccountDelegation(change) => AssetChange::AccountDelegation(DelegationChange {
                account: map(change.account)?,
                before: AccountDelegation {
                    delegate: change.before.delegate.map(&mut map).transpose()?,
                    nonce: change.before.nonce,
                },
                after: AccountDelegation {
                    delegate: change.after.delegate.map(&mut map).transpose()?,
                    nonce: change.after.nonce,
                },
            }),
            Self::WrappedNativeDeposit(change) => {
                AssetChange::WrappedNativeDeposit(WrappedNativeChange {
                    contract_address: map(change.contract_address)?,
                    account: map(change.account)?,
                    raw_amount: change.raw_amount,
                })
            }
            Self::WrappedNativeWithdrawal(change) => {
                AssetChange::WrappedNativeWithdrawal(WrappedNativeChange {
                    contract_address: map(change.contract_address)?,
                    account: map(change.account)?,
                    raw_amount: change.raw_amount,
                })
            }
            Self::Standard(change) => AssetChange::Standard(change.try_map_addresses(map)?),
        })
    }
    pub fn metadata_request(&self) -> Option<(A, MetadataKind)> {
        match self {
            Self::WrappedNativeDeposit(change) | Self::WrappedNativeWithdrawal(change) => {
                Some((change.contract_address.clone(), MetadataKind::Erc20))
            }
            Self::Standard(change) => change
                .metadata_kind()
                .map(|kind| (change.contract_address().clone(), kind)),
            _ => None,
        }
    }
    pub fn effect(&self) -> AssetEffect<A> {
        match self {
            Self::NativeTransfer(change) => AssetEffect::NativeTransfer {
                from: change.from.clone(),
                to: change.to.clone(),
            },
            Self::SelfDestructBurn(change) => {
                AssetEffect::NativeBurn(change.contract_address.clone())
            }
            Self::AccountDelegation(change) => AssetEffect::Delegation(change.account.clone()),
            Self::WrappedNativeDeposit(change) | Self::WrappedNativeWithdrawal(change) => {
                AssetEffect::Standard(StandardEffect::FungibleMovement(
                    change.contract_address.clone(),
                ))
            }
            Self::Standard(change) => AssetEffect::Standard(change.effect()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetChangeSet<A: Ord = Address> {
    space: Option<crate::analysis::ExecutionSpace>,
    changes: OrderedChanges<AssetEffect<A>, AssetChange<A>>,
    metadata: MetadataStore<A>,
}

impl<A: Ord> Default for AssetChangeSet<A> {
    fn default() -> Self {
        Self {
            space: None,
            changes: OrderedChanges::default(),
            metadata: MetadataStore::default(),
        }
    }
}

impl<A: Ord + Clone> AssetChangeSet<A> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_space(&mut self, space: crate::analysis::ExecutionSpace) {
        self.space = Some(space);
    }
    pub fn space(&self) -> Option<crate::analysis::ExecutionSpace> {
        self.space
    }
    pub fn insert(&mut self, position: impl Into<ChangePosition>, change: AssetChange<A>) {
        self.changes
            .insert(position.into(), change.effect(), change);
    }
    pub fn items(&self) -> impl ExactSizeIterator<Item = &AssetChange<A>> {
        self.changes.items()
    }
    pub fn into_items(self) -> Vec<AssetChange<A>> {
        self.changes.into_items()
    }
    pub fn iter(&self) -> impl Iterator<Item = (ChangePosition, &AssetChange<A>)> {
        self.changes.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
    pub fn metadata(&self) -> &MetadataStore<A> {
        &self.metadata
    }
}

impl<A: Ord + Clone> crate::analysis::MergeChanges for AssetChangeSet<A> {
    fn merge(self, other: Self) -> Self {
        Self {
            space: self.space.or(other.space),
            changes: self.changes.merge(other.changes),
            metadata: MetadataStore::default(),
        }
    }
}

impl<A: Ord + Clone> AssetChangeSet<A> {
    pub fn load_metadata<R: contract_standards::MetadataReader<A> + ?Sized>(
        &mut self,
        reader: &R,
    ) -> Result<(), R::Error> {
        self.metadata = contract_standards::load_metadata(
            reader,
            self.items().filter_map(AssetChange::metadata_request),
        )?;
        Ok(())
    }
}
