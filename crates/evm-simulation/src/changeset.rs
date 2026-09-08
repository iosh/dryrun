use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    sync::Arc,
};

use alloy::primitives::{Address, U256};
use contract_standards::{Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem};
use thiserror::Error;

use crate::{
    EvmObservationRequirements, EvmTokenChangeRules,
    execution::{
        EvmCallKind, EvmExecutionPosition, EvmFrameAction, EvmObservationError,
        EvmTransactionExecution,
    },
    state::{EvmStateAccess, EvmStateReadError},
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
    entries: Vec<EvmChangeEntry>,
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

    fn merge(self, other: Self) -> Result<Self, EvmChangeDerivationError> {
        let mut builder = EvmChangeSetBuilder::new();
        for entry in self.entries.into_iter().chain(other.entries) {
            builder.insert_entry(entry)?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvmChangeEntry {
    position: EvmChangePosition,
    change: EvmStateChange,
    metadata_conflicts: EvmMetadataConflicts,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct EvmMetadataConflicts {
    name: bool,
    symbol: bool,
    decimals: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EvmChangePosition {
    PreExecution(u8),
    Execution(EvmExecutionPosition),
    PostExecution(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmStateChange {
    NativeTransfer(EvmNativeTransferChange),
    SelfDestructBurn(EvmSelfDestructBurnChange),
    AccountDelegation(EvmAccountDelegationChange),
    WrappedNativeDeposit(EvmWrappedNativeDepositChange),
    WrappedNativeWithdrawal(EvmWrappedNativeWithdrawalChange),
    Standard(EvmStandardChange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmStandardChange {
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Mint {
        contract_address: Address,
        to: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Burn {
        contract_address: Address,
        from: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Approval {
        contract_address: Address,
        owner: Address,
        spender: Address,
        before: U256,
        after: U256,
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Mint {
        contract_address: Address,
        to: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Burn {
        contract_address: Address,
        from: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155MintSingle {
        contract_address: Address,
        operator: Address,
        to: Address,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155BurnSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<Erc1155TransferItem>,
    },
    Erc1155MintBatch {
        contract_address: Address,
        operator: Address,
        to: Address,
        items: Vec<Erc1155TransferItem>,
    },
    Erc1155BurnBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        items: Vec<Erc1155TransferItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmNativeTransferChange {
    pub from: Address,
    pub to: Address,
    pub raw_amount: U256,
    pub currency: EvmNativeCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSelfDestructBurnChange {
    pub contract_address: Address,
    pub raw_amount: U256,
    pub currency: EvmNativeCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmAccountDelegationChange {
    pub account: Address,
    pub before: EvmAccountDelegation,
    pub after: EvmAccountDelegation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmWrappedNativeDepositChange {
    pub contract_address: Address,
    pub account: Address,
    pub raw_amount: U256,
    pub metadata: contract_standards::Erc20Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmWrappedNativeWithdrawalChange {
    pub contract_address: Address,
    pub account: Address,
    pub raw_amount: U256,
    pub metadata: contract_standards::Erc20Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvmAccountDelegation {
    pub delegate: Option<Address>,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum EvmChangeKey {
    NativeTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        contract: Address,
        amount: U256,
    },
    AccountDelegation(Address),
    WrappedNativeDeposit {
        contract: Address,
        account: Address,
        amount: U256,
    },
    WrappedNativeWithdrawal {
        contract: Address,
        account: Address,
        amount: U256,
    },
    Standard(StandardChangeKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum StandardChangeKey {
    Erc20Transfer {
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Approval {
        contract: Address,
        owner: Address,
        spender: Address,
        before: U256,
        after: U256,
    },
    Erc20Mint {
        contract: Address,
        to: Address,
        amount: U256,
    },
    Erc20Burn {
        contract: Address,
        from: Address,
        amount: U256,
    },
    Erc721Transfer {
        contract: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc721Approval {
        contract: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        token_id: U256,
    },
    Erc721Mint {
        contract: Address,
        to: Address,
        token_id: U256,
    },
    Erc721Burn {
        contract: Address,
        from: Address,
        token_id: U256,
    },
    OperatorApproval {
        contract: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155TransferBatch {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<(U256, U256)>,
    },
    Erc1155MintSingle {
        contract: Address,
        operator: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155BurnSingle {
        contract: Address,
        operator: Address,
        from: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155MintBatch {
        contract: Address,
        operator: Address,
        to: Address,
        items: Vec<(U256, U256)>,
    },
    Erc1155BurnBatch {
        contract: Address,
        operator: Address,
        from: Address,
        items: Vec<(U256, U256)>,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmChangeDerivationError {
    #[error(transparent)]
    Observation(#[from] EvmObservationError),

    #[error(transparent)]
    StateRead(#[from] EvmStateReadError),

    #[error("conflicting changes: {details}")]
    Conflict { details: String },

    #[error("{rules} change rules could not derive complete changes: {source}")]
    RuleFailure {
        rules: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
}

impl EvmChangeDerivationError {
    pub fn rule_failure(
        rules: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::RuleFailure {
            rules,
            source: Box::new(source),
        }
    }

    fn conflict(details: impl Into<String>) -> Self {
        Self::Conflict {
            details: details.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct EvmChangeSetBuilder {
    entries: BTreeMap<(EvmChangePosition, EvmChangeKey), EvmChangeEntry>,
}

impl EvmChangeSetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert_at(
        &mut self,
        position: EvmChangePosition,
        item: EvmStateChange,
    ) -> Result<(), EvmChangeDerivationError> {
        self.insert_entry(EvmChangeEntry {
            position,
            change: item,
            metadata_conflicts: EvmMetadataConflicts::default(),
        })
    }

    fn insert_entry(&mut self, entry: EvmChangeEntry) -> Result<(), EvmChangeDerivationError> {
        let key = entry.change.key();
        let position = entry.position;
        let map_key = (position, key.clone());
        if let Some(existing) = self.entries.get_mut(&map_key) {
            if existing.merge_duplicate(entry) {
                return Ok(());
            }
            let details = match key {
                EvmChangeKey::AccountDelegation(account) => {
                    format!("different delegation values for account {account}")
                }
                _ => format!(
                    "different semantic values at execution position {}",
                    position.index()
                ),
            };
            return Err(EvmChangeDerivationError::conflict(details));
        }
        if matches!(position, EvmChangePosition::Execution(_))
            && self
                .entries
                .keys()
                .any(|(existing_position, _)| *existing_position == position)
        {
            return Err(EvmChangeDerivationError::conflict(format!(
                "multiple semantic changes at execution position {}",
                position.index()
            )));
        }
        self.entries.insert(map_key, entry);
        Ok(())
    }

    pub fn native_transfer(
        &mut self,
        position: EvmExecutionPosition,
        from: Address,
        to: Address,
        raw_amount: U256,
        currency: EvmNativeCurrency,
    ) -> Result<(), EvmChangeDerivationError> {
        if raw_amount.is_zero() || from == to {
            return Ok(());
        }
        self.insert_at(
            EvmChangePosition::Execution(position),
            EvmStateChange::NativeTransfer(EvmNativeTransferChange {
                from,
                to,
                raw_amount,
                currency,
            }),
        )
    }

    pub fn selfdestruct_burn(
        &mut self,
        position: EvmExecutionPosition,
        contract_address: Address,
        raw_amount: U256,
        currency: EvmNativeCurrency,
    ) -> Result<(), EvmChangeDerivationError> {
        if raw_amount.is_zero() {
            return Ok(());
        }
        self.insert_at(
            EvmChangePosition::Execution(position),
            EvmStateChange::SelfDestructBurn(EvmSelfDestructBurnChange {
                contract_address,
                raw_amount,
                currency,
            }),
        )
    }

    pub fn account_delegation(
        &mut self,
        account: Address,
        before: EvmAccountDelegation,
        after: EvmAccountDelegation,
    ) -> Result<(), EvmChangeDerivationError> {
        if before != after {
            self.insert_at(
                EvmChangePosition::PreExecution(0),
                EvmStateChange::AccountDelegation(EvmAccountDelegationChange {
                    account,
                    before,
                    after,
                }),
            )?;
        }
        Ok(())
    }

    pub(crate) fn standard(
        &mut self,
        position: EvmExecutionPosition,
        change: EvmStandardChange,
    ) -> Result<(), EvmChangeDerivationError> {
        self.insert_at(
            EvmChangePosition::Execution(position),
            EvmStateChange::Standard(change),
        )
    }

    pub(crate) fn wrapped_native_deposit(
        &mut self,
        position: EvmExecutionPosition,
        change: EvmWrappedNativeDepositChange,
    ) -> Result<(), EvmChangeDerivationError> {
        self.insert_at(
            EvmChangePosition::Execution(position),
            EvmStateChange::WrappedNativeDeposit(change),
        )
    }

    pub(crate) fn wrapped_native_withdrawal(
        &mut self,
        position: EvmExecutionPosition,
        change: EvmWrappedNativeWithdrawalChange,
    ) -> Result<(), EvmChangeDerivationError> {
        self.insert_at(
            EvmChangePosition::Execution(position),
            EvmStateChange::WrappedNativeWithdrawal(change),
        )
    }

    pub fn finish(self) -> EvmChangeSet {
        let entries = self.entries.into_values().collect::<Vec<_>>();
        let items = entries.iter().map(|entry| entry.change.clone()).collect();
        EvmChangeSet { items, entries }
    }
}

impl EvmStateChange {
    fn key(&self) -> EvmChangeKey {
        match self {
            Self::NativeTransfer(change) => EvmChangeKey::NativeTransfer {
                from: change.from,
                to: change.to,
                amount: change.raw_amount,
            },
            Self::SelfDestructBurn(change) => EvmChangeKey::SelfDestructBurn {
                contract: change.contract_address,
                amount: change.raw_amount,
            },
            Self::AccountDelegation(change) => EvmChangeKey::AccountDelegation(change.account),
            Self::WrappedNativeDeposit(change) => EvmChangeKey::WrappedNativeDeposit {
                contract: change.contract_address,
                account: change.account,
                amount: change.raw_amount,
            },
            Self::WrappedNativeWithdrawal(change) => EvmChangeKey::WrappedNativeWithdrawal {
                contract: change.contract_address,
                account: change.account,
                amount: change.raw_amount,
            },
            Self::Standard(change) => EvmChangeKey::Standard(StandardChangeKey::from(change)),
        }
    }
}

impl EvmChangeEntry {
    fn merge_duplicate(&mut self, other: Self) -> bool {
        match (&mut self.change, other.change) {
            (
                EvmStateChange::WrappedNativeDeposit(existing),
                EvmStateChange::WrappedNativeDeposit(incoming),
            ) => {
                merge_erc20_metadata(
                    &mut existing.metadata,
                    incoming.metadata,
                    &mut self.metadata_conflicts,
                    other.metadata_conflicts,
                );
                true
            }
            (
                EvmStateChange::WrappedNativeWithdrawal(existing),
                EvmStateChange::WrappedNativeWithdrawal(incoming),
            ) => {
                merge_erc20_metadata(
                    &mut existing.metadata,
                    incoming.metadata,
                    &mut self.metadata_conflicts,
                    other.metadata_conflicts,
                );
                true
            }
            (EvmStateChange::Standard(existing), EvmStateChange::Standard(incoming)) => {
                merge_standard_metadata(
                    existing,
                    incoming,
                    &mut self.metadata_conflicts,
                    other.metadata_conflicts,
                )
            }
            (existing, incoming) => *existing == incoming,
        }
    }
}

fn merge_standard_metadata(
    existing: &mut EvmStandardChange,
    incoming: EvmStandardChange,
    conflicts: &mut EvmMetadataConflicts,
    incoming_conflicts: EvmMetadataConflicts,
) -> bool {
    match (existing, incoming) {
        (
            EvmStandardChange::Erc20Transfer { metadata, .. }
            | EvmStandardChange::Erc20Mint { metadata, .. }
            | EvmStandardChange::Erc20Burn { metadata, .. },
            EvmStandardChange::Erc20Transfer {
                metadata: incoming, ..
            }
            | EvmStandardChange::Erc20Mint {
                metadata: incoming, ..
            }
            | EvmStandardChange::Erc20Burn {
                metadata: incoming, ..
            },
        ) => {
            merge_erc20_metadata(metadata, incoming, conflicts, incoming_conflicts);
            true
        }
        (
            EvmStandardChange::Erc20Approval { metadata, .. },
            EvmStandardChange::Erc20Approval {
                metadata: incoming, ..
            },
        ) => {
            merge_erc20_metadata(metadata, incoming, conflicts, incoming_conflicts);
            true
        }
        (
            EvmStandardChange::Erc721Transfer { metadata, .. }
            | EvmStandardChange::Erc721Mint { metadata, .. }
            | EvmStandardChange::Erc721Burn { metadata, .. },
            EvmStandardChange::Erc721Transfer {
                metadata: incoming, ..
            }
            | EvmStandardChange::Erc721Mint {
                metadata: incoming, ..
            }
            | EvmStandardChange::Erc721Burn {
                metadata: incoming, ..
            },
        ) => {
            merge_metadata_field(
                &mut metadata.name,
                incoming.name,
                &mut conflicts.name,
                incoming_conflicts.name,
            );
            merge_metadata_field(
                &mut metadata.symbol,
                incoming.symbol,
                &mut conflicts.symbol,
                incoming_conflicts.symbol,
            );
            true
        }
        (
            EvmStandardChange::Erc721Approval { metadata, .. },
            EvmStandardChange::Erc721Approval {
                metadata: incoming, ..
            },
        ) => {
            merge_metadata_field(
                &mut metadata.name,
                incoming.name,
                &mut conflicts.name,
                incoming_conflicts.name,
            );
            merge_metadata_field(
                &mut metadata.symbol,
                incoming.symbol,
                &mut conflicts.symbol,
                incoming_conflicts.symbol,
            );
            true
        }
        (existing, incoming) => *existing == incoming,
    }
}

fn merge_erc20_metadata(
    existing: &mut contract_standards::Erc20Metadata,
    incoming: contract_standards::Erc20Metadata,
    conflicts: &mut EvmMetadataConflicts,
    incoming_conflicts: EvmMetadataConflicts,
) {
    merge_metadata_field(
        &mut existing.name,
        incoming.name,
        &mut conflicts.name,
        incoming_conflicts.name,
    );
    merge_metadata_field(
        &mut existing.symbol,
        incoming.symbol,
        &mut conflicts.symbol,
        incoming_conflicts.symbol,
    );
    merge_metadata_field(
        &mut existing.decimals,
        incoming.decimals,
        &mut conflicts.decimals,
        incoming_conflicts.decimals,
    );
}

fn merge_metadata_field<T: Eq>(
    existing: &mut Option<T>,
    incoming: Option<T>,
    conflict: &mut bool,
    incoming_conflict: bool,
) {
    if *conflict || incoming_conflict {
        *conflict = true;
        *existing = None;
        return;
    }

    match (existing.as_ref(), incoming) {
        (None, Some(value)) => *existing = Some(value),
        (Some(current), Some(value)) if current != &value => {
            *conflict = true;
            *existing = None;
        }
        _ => {}
    }
}

impl StandardChangeKey {
    fn from(change: &EvmStandardChange) -> Self {
        match change {
            EvmStandardChange::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                ..
            } => Self::Erc20Transfer {
                contract: *contract_address,
                from: *from,
                to: *to,
                amount: *raw_amount,
            },
            EvmStandardChange::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                ..
            } => Self::Erc20Mint {
                contract: *contract_address,
                to: *to,
                amount: *raw_amount,
            },
            EvmStandardChange::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                ..
            } => Self::Erc20Burn {
                contract: *contract_address,
                from: *from,
                amount: *raw_amount,
            },
            EvmStandardChange::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
                ..
            } => Self::Erc20Approval {
                contract: *contract_address,
                owner: *owner,
                spender: *spender,
                before: *before,
                after: *after,
            },
            EvmStandardChange::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                ..
            } => Self::Erc721Transfer {
                contract: *contract_address,
                from: *from,
                to: *to,
                token_id: *token_id,
            },
            EvmStandardChange::Erc721Mint {
                contract_address,
                to,
                token_id,
                ..
            } => Self::Erc721Mint {
                contract: *contract_address,
                to: *to,
                token_id: *token_id,
            },
            EvmStandardChange::Erc721Burn {
                contract_address,
                from,
                token_id,
                ..
            } => Self::Erc721Burn {
                contract: *contract_address,
                from: *from,
                token_id: *token_id,
            },
            EvmStandardChange::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
                ..
            } => Self::Erc721Approval {
                contract: *contract_address,
                owner: *owner,
                before: *before,
                after: *after,
                token_id: *token_id,
            },
            EvmStandardChange::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            } => Self::OperatorApproval {
                contract: *contract_address,
                owner: *owner,
                operator: *operator,
                before: *before,
                after: *after,
            },
            EvmStandardChange::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155TransferSingle {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                to: *to,
                token_id: *token_id,
                amount: *raw_amount,
            },
            EvmStandardChange::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155MintSingle {
                contract: *contract_address,
                operator: *operator,
                to: *to,
                token_id: *token_id,
                amount: *raw_amount,
            },
            EvmStandardChange::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            } => Self::Erc1155BurnSingle {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                token_id: *token_id,
                amount: *raw_amount,
            },
            EvmStandardChange::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => Self::Erc1155TransferBatch {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                to: *to,
                items: items
                    .iter()
                    .map(|item| (item.token_id, item.raw_amount))
                    .collect(),
            },
            EvmStandardChange::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items,
            } => Self::Erc1155MintBatch {
                contract: *contract_address,
                operator: *operator,
                to: *to,
                items: items
                    .iter()
                    .map(|item| (item.token_id, item.raw_amount))
                    .collect(),
            },
            EvmStandardChange::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items,
            } => Self::Erc1155BurnBatch {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                items: items
                    .iter()
                    .map(|item| (item.token_id, item.raw_amount))
                    .collect(),
            },
        }
    }
}

impl EvmChangePosition {
    const fn index(self) -> usize {
        match self {
            Self::PreExecution(index) => index as usize,
            Self::Execution(position) => position.index(),
            Self::PostExecution(index) => usize::MAX - index as usize,
        }
    }
}

pub trait EvmChangeRules: Send + Sync + 'static {
    fn required_observations(&self) -> EvmObservationRequirements;

    fn derive_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeDerivationError>;

    fn combine<R>(self, other: R) -> CombinedEvmChangeRules<Self, R>
    where
        Self: Sized,
        R: EvmChangeRules,
    {
        CombinedEvmChangeRules::new(self, other)
    }
}

#[derive(Debug)]
pub enum EvmChanges {
    Complete(EvmChangeSet),
    Unavailable(EvmChangeDerivationError),
}

impl From<Result<EvmChangeSet, EvmChangeDerivationError>> for EvmChanges {
    fn from(result: Result<EvmChangeSet, EvmChangeDerivationError>) -> Self {
        match result {
            Ok(changes) => Self::Complete(changes),
            Err(error) => Self::Unavailable(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvmNativeOperation {
    Transfer {
        position: EvmExecutionPosition,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        position: EvmExecutionPosition,
        contract: Address,
        amount: U256,
    },
}

#[derive(Debug, Error)]
#[error("{details}")]
struct NativeAssetChangeError {
    details: String,
}

impl NativeAssetChangeError {
    fn new(details: impl Into<String>) -> Self {
        Self {
            details: details.into(),
        }
    }
}

fn native_asset_error(details: impl Into<String>) -> EvmChangeDerivationError {
    EvmChangeDerivationError::rule_failure("native asset", NativeAssetChangeError::new(details))
}

#[derive(Debug, Clone)]
pub struct EvmNativeAssetChangeRules {
    currency: EvmNativeCurrency,
}

impl EvmNativeAssetChangeRules {
    pub fn new(currency: EvmNativeCurrency) -> Self {
        Self { currency }
    }
}

impl EvmChangeRules for EvmNativeAssetChangeRules {
    fn required_observations(&self) -> EvmObservationRequirements {
        EvmObservationRequirements::new()
    }

    fn derive_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeDerivationError> {
        let operations = collect_native_operations(execution);
        replay_native_balances(execution, state, &operations)?;

        let mut builder = EvmChangeSetBuilder::new();
        for operation in operations {
            match operation {
                EvmNativeOperation::Transfer {
                    position,
                    from,
                    to,
                    amount,
                } => builder.native_transfer(position, from, to, amount, self.currency.clone())?,
                EvmNativeOperation::SelfDestructBurn {
                    position,
                    contract,
                    amount,
                } => {
                    builder.selfdestruct_burn(position, contract, amount, self.currency.clone())?
                }
            }
        }

        Ok(builder.finish())
    }
}

fn collect_native_operations(execution: &EvmTransactionExecution) -> Vec<EvmNativeOperation> {
    let mut operations = Vec::new();

    for frame in execution.committed_frames() {
        match frame.action() {
            EvmFrameAction::Call {
                kind: EvmCallKind::Call,
                caller,
                target,
                value,
                ..
            } if !value.is_zero() && caller != target => {
                operations.push(EvmNativeOperation::Transfer {
                    position: frame.position(),
                    from: *caller,
                    to: *target,
                    amount: *value,
                });
            }
            EvmFrameAction::Create {
                caller,
                value,
                created_address,
                ..
            } if !value.is_zero() => {
                let to = created_address.unwrap_or_else(|| {
                    unreachable!(
                        "successful CREATE frame must have an address after execution commit"
                    )
                });
                if *caller != to {
                    operations.push(EvmNativeOperation::Transfer {
                        position: frame.position(),
                        from: *caller,
                        to,
                        amount: *value,
                    });
                }
            }
            EvmFrameAction::Call { .. } | EvmFrameAction::Create { .. } => {}
        }
    }

    for selfdestruct in execution.committed_selfdestructs() {
        let amount = selfdestruct.value();
        if amount.is_zero() {
            continue;
        }

        if selfdestruct.contract() == selfdestruct.target() {
            operations.push(EvmNativeOperation::SelfDestructBurn {
                position: selfdestruct.position(),
                contract: selfdestruct.contract(),
                amount,
            });
        } else {
            operations.push(EvmNativeOperation::Transfer {
                position: selfdestruct.position(),
                from: selfdestruct.contract(),
                to: selfdestruct.target(),
                amount,
            });
        }
    }

    operations.sort_by_key(|operation| match operation {
        EvmNativeOperation::Transfer { position, .. }
        | EvmNativeOperation::SelfDestructBurn { position, .. } => *position,
    });
    operations
}

fn replay_native_balances(
    execution: &EvmTransactionExecution,
    state: &EvmStateAccess,
    operations: &[EvmNativeOperation],
) -> Result<(), EvmChangeDerivationError> {
    let mut addresses = BTreeSet::new();
    addresses.insert(execution.fee_payer());
    addresses.insert(execution.block_beneficiary());
    for operation in operations {
        match operation {
            EvmNativeOperation::Transfer { from, to, .. } => {
                addresses.insert(*from);
                addresses.insert(*to);
            }
            EvmNativeOperation::SelfDestructBurn { contract, .. } => {
                addresses.insert(*contract);
            }
        }
    }

    let mut replayed = BTreeMap::new();
    for address in addresses {
        let balance = state.initial().read_account(address)?.balance();
        replayed.insert(address, balance);
    }

    decrease_native_balance(
        &mut replayed,
        execution.fee_payer(),
        execution.fee().total_charged_amount(),
    )?;

    for operation in operations {
        match operation {
            EvmNativeOperation::Transfer {
                from, to, amount, ..
            } => {
                decrease_native_balance(&mut replayed, *from, *amount)?;
                increase_native_balance(&mut replayed, *to, *amount)?;
            }
            EvmNativeOperation::SelfDestructBurn {
                contract, amount, ..
            } => {
                decrease_native_balance(&mut replayed, *contract, *amount)?;
            }
        }
    }

    increase_native_balance(
        &mut replayed,
        execution.block_beneficiary(),
        execution.fee().beneficiary_reward(),
    )?;

    for (&address, &replayed_balance) in &replayed {
        let actual = state.finalized().read_account(address)?.balance();
        if replayed_balance != actual {
            return Err(native_asset_error(format!(
                "finalized native balance differs from replay for {address}: replayed {replayed_balance}, state {actual}"
            )));
        }
    }

    Ok(())
}

fn decrease_native_balance(
    balances: &mut BTreeMap<Address, U256>,
    address: Address,
    amount: U256,
) -> Result<(), EvmChangeDerivationError> {
    let balance = balances.get_mut(&address).unwrap_or_else(|| {
        unreachable!("native replay address set must contain every operation account")
    });
    let current = *balance;
    *balance = current.checked_sub(amount).ok_or_else(|| {
        native_asset_error(format!(
            "native balance replay violated the execution invariant for {address}"
        ))
    })?;
    Ok(())
}

fn increase_native_balance(
    balances: &mut BTreeMap<Address, U256>,
    address: Address,
    amount: U256,
) -> Result<(), EvmChangeDerivationError> {
    let balance = balances.get_mut(&address).unwrap_or_else(|| {
        unreachable!("native replay address set must contain every operation account")
    });
    let current = *balance;
    *balance = current.checked_add(amount).ok_or_else(|| {
        native_asset_error(format!(
            "native balance replay violated the execution invariant for {address}"
        ))
    })?;
    Ok(())
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EvmAccountDelegationChangeRules;

impl EvmChangeRules for EvmAccountDelegationChangeRules {
    fn required_observations(&self) -> EvmObservationRequirements {
        EvmObservationRequirements::new()
    }

    fn derive_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeDerivationError> {
        let mut builder = EvmChangeSetBuilder::new();
        for &account in execution.applied_authorization_accounts() {
            let before = state.initial().read_account(account)?;
            let after = state.finalized().read_account(account)?;
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
pub struct DefaultEvmChangeRules {
    components: CombinedEvmChangeRules<
        CombinedEvmChangeRules<EvmNativeAssetChangeRules, EvmAccountDelegationChangeRules>,
        EvmTokenChangeRules,
    >,
}

impl DefaultEvmChangeRules {
    pub fn new(currency: EvmNativeCurrency) -> Self {
        Self::with_wrapped_native_token(currency, None)
    }

    pub(crate) fn with_wrapped_native_token(
        currency: EvmNativeCurrency,
        wrapped_native_token: Option<Address>,
    ) -> Self {
        Self {
            components: CombinedEvmChangeRules::new(
                CombinedEvmChangeRules::new(
                    EvmNativeAssetChangeRules::new(currency),
                    EvmAccountDelegationChangeRules,
                ),
                EvmTokenChangeRules::new(wrapped_native_token),
            ),
        }
    }
}

impl EvmChangeRules for DefaultEvmChangeRules {
    fn required_observations(&self) -> EvmObservationRequirements {
        self.components.required_observations()
    }

    fn derive_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeDerivationError> {
        self.components.derive_changes(execution, state)
    }
}

#[derive(Debug, Clone)]
pub struct CombinedEvmChangeRules<A, B> {
    first: Arc<A>,
    second: Arc<B>,
}

impl<A, B> CombinedEvmChangeRules<A, B> {
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

impl<A, B> EvmChangeRules for CombinedEvmChangeRules<A, B>
where
    A: EvmChangeRules,
    B: EvmChangeRules,
{
    fn required_observations(&self) -> EvmObservationRequirements {
        self.first
            .required_observations()
            .merge(&self.second.required_observations())
    }

    fn derive_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeDerivationError> {
        let first = self.first.derive_changes(execution, state)?;
        let second = self.second.derive_changes(execution, state)?;
        first.merge(second)
    }
}
