mod native;
mod standards;
mod wrapped_native;

use std::{collections::BTreeMap, error::Error as StdError, sync::Arc};

use alloy_primitives::{Address, U256};
use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, MetadataCall, StandardChange,
    metadata_calls,
};
use thiserror::Error;

use crate::execution::{CommittedExecutionTrace, LogCheckpoint};

use self::{
    standards::{DecodedStandardOccurrence, decode_standard_occurrences_in_scope},
    wrapped_native::{WrappedNativeOccurrence, decode_wrapped_native_occurrences_in_scope},
};
use super::{
    EspaceAccountState, EspaceChangesError, EspaceExecutedTransaction, EspaceExecutionPosition,
    EspaceStateAccess,
};

pub(crate) use standards::{
    IsolatedReadCallError, MetadataReadError, ReadCallOutcome, execute_isolated_read_call,
    execute_read_call,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceNativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceChange {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
        currency: EspaceNativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        raw_amount: U256,
        currency: EspaceNativeCurrency,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Standard(StandardChange<Address>),
}

/// A verified standalone eSpace change.  `EspaceChange` below is retained for
/// Core Space's legacy nested analysis and must not be used at the standalone
/// RPC boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceStateChange {
    NativeTransfer(EspaceNativeTransferChange),
    SelfDestructBurn(EspaceSelfDestructBurnChange),
    WrappedNativeDeposit(EspaceWrappedNativeDepositChange),
    WrappedNativeWithdrawal(EspaceWrappedNativeWithdrawalChange),
    Standard(EspaceStandardChange),
    AccountDelegation(EspaceAccountDelegationChange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceNativeTransferChange {
    pub from: Address,
    pub to: Address,
    pub raw_amount: U256,
    pub currency: EspaceNativeCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSelfDestructBurnChange {
    pub contract_address: Address,
    pub raw_amount: U256,
    pub currency: EspaceNativeCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceWrappedNativeDepositChange {
    pub contract_address: Address,
    pub account: Address,
    pub raw_amount: U256,
    pub metadata: Erc20Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceWrappedNativeWithdrawalChange {
    pub contract_address: Address,
    pub account: Address,
    pub raw_amount: U256,
    pub metadata: Erc20Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceAccountDelegationChange {
    pub account: Address,
    pub before: EspaceAccountDelegation,
    pub after: EspaceAccountDelegation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EspaceAccountDelegation {
    pub delegate: Option<Address>,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceStandardChange {
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

/// The verified changes produced by one finalized eSpace execution.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EspaceChangeSet {
    items: Vec<EspaceStateChange>,
    entries: Vec<EspaceChangeEntry>,
}

impl EspaceChangeSet {
    pub fn items(&self) -> &[EspaceStateChange] {
        &self.items
    }

    pub fn into_items(self) -> Vec<EspaceStateChange> {
        self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn merge(self, other: Self) -> Result<Self, EspaceChangeDerivationError> {
        let mut builder = EspaceChangeSetBuilder::new();
        for entry in self.entries.into_iter().chain(other.entries) {
            builder.insert_entry(entry)?;
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EspaceChangeEntry {
    position: EspaceChangePosition,
    change: EspaceStateChange,
    metadata_conflicts: EspaceMetadataConflicts,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct EspaceMetadataConflicts {
    name: bool,
    symbol: bool,
    decimals: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EspaceChangePosition {
    PreExecution,
    Execution(EspaceExecutionPosition),
}

impl EspaceChangePosition {
    fn index(self) -> usize {
        match self {
            Self::PreExecution => 0,
            Self::Execution(position) => position.index().saturating_add(1),
        }
    }
}

/// Errors raised while a change-rule component is deriving a complete set.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceChangeDerivationError {
    #[error(transparent)]
    Existing(#[from] EspaceChangesError),

    #[error("conflicting eSpace changes: {details}")]
    Conflict { details: String },

    #[error("{rules} change rules could not derive complete changes: {source}")]
    RuleFailure {
        rules: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
}

impl EspaceChangeDerivationError {
    pub fn rule_failure(
        rules: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::RuleFailure {
            rules,
            source: Box::new(source),
        }
    }
}

/// A change result is either complete (including a verified empty set) or
/// unavailable because the required evidence could not be established.
#[derive(Debug)]
pub enum EspaceChanges {
    Complete(EspaceChangeSet),
    Unavailable(EspaceChangeDerivationError),
}

impl From<Result<EspaceChangeSet, EspaceChangeDerivationError>> for EspaceChanges {
    fn from(result: Result<EspaceChangeSet, EspaceChangeDerivationError>) -> Self {
        match result {
            Ok(changes) => Self::Complete(changes),
            Err(error) => Self::Unavailable(error),
        }
    }
}

/// Log/state observations requested by a change-rule component before the
/// single formal VM execution starts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EspaceObservationRequirements {
    log_checkpoints: Vec<EspaceLogCheckpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EspaceLogCheckpoint {
    address: Option<Address>,
    topic0: alloy_primitives::B256,
}

impl EspaceObservationRequirements {
    pub const fn new() -> Self {
        Self {
            log_checkpoints: Vec::new(),
        }
    }

    pub fn checkpoint_any_address(&mut self, topic0: alloy_primitives::B256) {
        self.insert(EspaceLogCheckpoint {
            address: None,
            topic0,
        });
    }

    pub fn checkpoint_at(&mut self, address: Address, topic0: alloy_primitives::B256) {
        self.insert(EspaceLogCheckpoint {
            address: Some(address),
            topic0,
        });
    }

    fn insert(&mut self, checkpoint: EspaceLogCheckpoint) {
        if !self.log_checkpoints.contains(&checkpoint) {
            self.log_checkpoints.push(checkpoint);
        }
    }

    pub(crate) fn into_log_checkpoints(self) -> Vec<LogCheckpoint> {
        self.log_checkpoints
            .into_iter()
            .map(|checkpoint| LogCheckpoint {
                space: cfx_types::Space::Ethereum,
                address: checkpoint.address.map(crate::primitive::address_to_cfx),
                topic0: crate::primitive::b256_to_cfx(checkpoint.topic0),
            })
            .collect()
    }

    fn merge(&mut self, other: Self) {
        for checkpoint in other.log_checkpoints {
            self.insert(checkpoint);
        }
    }
}

/// A replaceable, statically composable eSpace change-rule component.
pub trait EspaceChangeRules: Send + Sync + 'static {
    fn required_observations(&self) -> EspaceObservationRequirements;

    fn derive_changes(
        &self,
        execution: &EspaceExecutedTransaction,
        state: &EspaceStateAccess,
    ) -> Result<EspaceChangeSet, EspaceChangeDerivationError>;

    fn combine<R>(self, other: R) -> CombinedEspaceChangeRules<Self, R>
    where
        Self: Sized,
        R: EspaceChangeRules,
    {
        CombinedEspaceChangeRules::new(self, other)
    }
}

#[derive(Debug, Clone)]
pub struct CombinedEspaceChangeRules<A, B> {
    first: Arc<A>,
    second: Arc<B>,
}

impl<A, B> CombinedEspaceChangeRules<A, B> {
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

impl<A, B> EspaceChangeRules for CombinedEspaceChangeRules<A, B>
where
    A: EspaceChangeRules,
    B: EspaceChangeRules,
{
    fn required_observations(&self) -> EspaceObservationRequirements {
        let mut requirements = self.first.required_observations();
        requirements.merge(self.second.required_observations());
        requirements
    }

    fn derive_changes(
        &self,
        execution: &EspaceExecutedTransaction,
        state: &EspaceStateAccess,
    ) -> Result<EspaceChangeSet, EspaceChangeDerivationError> {
        let first = self.first.derive_changes(execution, state)?;
        let second = self.second.derive_changes(execution, state)?;
        first.merge(second)
    }
}

/// Builder used by custom rules and by the built-in eSpace components.
#[derive(Debug, Default)]
pub struct EspaceChangeSetBuilder {
    entries: BTreeMap<(EspaceChangePosition, EspaceChangeKey), EspaceChangeEntry>,
}

impl EspaceChangeSetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert_entry(
        &mut self,
        entry: EspaceChangeEntry,
    ) -> Result<(), EspaceChangeDerivationError> {
        let key = entry.change.key();
        let position = entry.position;
        let map_key = (position, key.clone());
        if let Some(existing) = self.entries.get_mut(&map_key) {
            if existing.merge_duplicate(entry) {
                return Ok(());
            }
            return Err(EspaceChangeDerivationError::Conflict {
                details: format!(
                    "different semantic values at execution position {}",
                    position.index()
                ),
            });
        }
        if matches!(position, EspaceChangePosition::Execution(_))
            && self
                .entries
                .keys()
                .any(|(existing_position, _)| *existing_position == position)
        {
            return Err(EspaceChangeDerivationError::Conflict {
                details: format!(
                    "multiple semantic changes at execution position {}",
                    position.index()
                ),
            });
        }
        self.entries.insert(map_key, entry);
        Ok(())
    }

    pub fn native_transfer(
        &mut self,
        position: EspaceExecutionPosition,
        from: Address,
        to: Address,
        raw_amount: U256,
        currency: EspaceNativeCurrency,
    ) -> Result<(), EspaceChangeDerivationError> {
        if raw_amount.is_zero() || from == to {
            return Ok(());
        }
        self.insert_entry(EspaceChangeEntry {
            position: EspaceChangePosition::Execution(position),
            change: EspaceStateChange::NativeTransfer(EspaceNativeTransferChange {
                from,
                to,
                raw_amount,
                currency,
            }),
            metadata_conflicts: EspaceMetadataConflicts::default(),
        })
    }

    pub fn selfdestruct_burn(
        &mut self,
        position: EspaceExecutionPosition,
        contract_address: Address,
        raw_amount: U256,
        currency: EspaceNativeCurrency,
    ) -> Result<(), EspaceChangeDerivationError> {
        if raw_amount.is_zero() {
            return Ok(());
        }
        self.insert_entry(EspaceChangeEntry {
            position: EspaceChangePosition::Execution(position),
            change: EspaceStateChange::SelfDestructBurn(EspaceSelfDestructBurnChange {
                contract_address,
                raw_amount,
                currency,
            }),
            metadata_conflicts: EspaceMetadataConflicts::default(),
        })
    }

    pub fn standard(
        &mut self,
        position: EspaceExecutionPosition,
        change: EspaceStandardChange,
    ) -> Result<(), EspaceChangeDerivationError> {
        self.insert_entry(EspaceChangeEntry {
            position: EspaceChangePosition::Execution(position),
            change: EspaceStateChange::Standard(change),
            metadata_conflicts: EspaceMetadataConflicts::default(),
        })
    }

    pub fn wrapped_native_deposit(
        &mut self,
        position: EspaceExecutionPosition,
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    ) -> Result<(), EspaceChangeDerivationError> {
        self.insert_entry(EspaceChangeEntry {
            position: EspaceChangePosition::Execution(position),
            change: EspaceStateChange::WrappedNativeDeposit(EspaceWrappedNativeDepositChange {
                contract_address,
                account,
                raw_amount,
                metadata,
            }),
            metadata_conflicts: EspaceMetadataConflicts::default(),
        })
    }

    pub fn wrapped_native_withdrawal(
        &mut self,
        position: EspaceExecutionPosition,
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    ) -> Result<(), EspaceChangeDerivationError> {
        self.insert_entry(EspaceChangeEntry {
            position: EspaceChangePosition::Execution(position),
            change: EspaceStateChange::WrappedNativeWithdrawal(
                EspaceWrappedNativeWithdrawalChange {
                    contract_address,
                    account,
                    raw_amount,
                    metadata,
                },
            ),
            metadata_conflicts: EspaceMetadataConflicts::default(),
        })
    }

    pub fn account_delegation(
        &mut self,
        account: Address,
        before: EspaceAccountDelegation,
        after: EspaceAccountDelegation,
    ) -> Result<(), EspaceChangeDerivationError> {
        if before == after {
            return Ok(());
        }
        self.insert_entry(EspaceChangeEntry {
            position: EspaceChangePosition::PreExecution,
            change: EspaceStateChange::AccountDelegation(EspaceAccountDelegationChange {
                account,
                before,
                after,
            }),
            metadata_conflicts: EspaceMetadataConflicts::default(),
        })
    }

    pub fn finish(self) -> EspaceChangeSet {
        let entries = self.entries.into_values().collect::<Vec<_>>();
        let items = entries.iter().map(|entry| entry.change.clone()).collect();
        EspaceChangeSet { items, entries }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum EspaceChangeKey {
    NativeTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        contract: Address,
        amount: U256,
    },
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
    AccountDelegation(Address),
    Standard(EspaceStandardChangeKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum EspaceStandardChangeKey {
    Erc20Transfer {
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
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
    Erc20Approval {
        contract: Address,
        owner: Address,
        spender: Address,
        before: U256,
        after: U256,
    },
    Erc721Transfer {
        contract: Address,
        from: Address,
        to: Address,
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
    Erc721Approval {
        contract: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
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
    Erc1155TransferBatch {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<(U256, U256)>,
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

impl EspaceStateChange {
    fn key(&self) -> EspaceChangeKey {
        match self {
            Self::NativeTransfer(change) => EspaceChangeKey::NativeTransfer {
                from: change.from,
                to: change.to,
                amount: change.raw_amount,
            },
            Self::SelfDestructBurn(change) => EspaceChangeKey::SelfDestructBurn {
                contract: change.contract_address,
                amount: change.raw_amount,
            },
            Self::WrappedNativeDeposit(change) => EspaceChangeKey::WrappedNativeDeposit {
                contract: change.contract_address,
                account: change.account,
                amount: change.raw_amount,
            },
            Self::WrappedNativeWithdrawal(change) => EspaceChangeKey::WrappedNativeWithdrawal {
                contract: change.contract_address,
                account: change.account,
                amount: change.raw_amount,
            },
            Self::AccountDelegation(change) => EspaceChangeKey::AccountDelegation(change.account),
            Self::Standard(change) => EspaceChangeKey::Standard(change.key()),
        }
    }
}

impl EspaceStandardChange {
    fn key(&self) -> EspaceStandardChangeKey {
        match self {
            Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                ..
            } => EspaceStandardChangeKey::Erc20Transfer {
                contract: *contract_address,
                from: *from,
                to: *to,
                amount: *raw_amount,
            },
            Self::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                ..
            } => EspaceStandardChangeKey::Erc20Mint {
                contract: *contract_address,
                to: *to,
                amount: *raw_amount,
            },
            Self::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                ..
            } => EspaceStandardChangeKey::Erc20Burn {
                contract: *contract_address,
                from: *from,
                amount: *raw_amount,
            },
            Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
                ..
            } => EspaceStandardChangeKey::Erc20Approval {
                contract: *contract_address,
                owner: *owner,
                spender: *spender,
                before: *before,
                after: *after,
            },
            Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                ..
            } => EspaceStandardChangeKey::Erc721Transfer {
                contract: *contract_address,
                from: *from,
                to: *to,
                token_id: *token_id,
            },
            Self::Erc721Mint {
                contract_address,
                to,
                token_id,
                ..
            } => EspaceStandardChangeKey::Erc721Mint {
                contract: *contract_address,
                to: *to,
                token_id: *token_id,
            },
            Self::Erc721Burn {
                contract_address,
                from,
                token_id,
                ..
            } => EspaceStandardChangeKey::Erc721Burn {
                contract: *contract_address,
                from: *from,
                token_id: *token_id,
            },
            Self::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
                ..
            } => EspaceStandardChangeKey::Erc721Approval {
                contract: *contract_address,
                owner: *owner,
                before: *before,
                after: *after,
                token_id: *token_id,
            },
            Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            } => EspaceStandardChangeKey::OperatorApproval {
                contract: *contract_address,
                owner: *owner,
                operator: *operator,
                before: *before,
                after: *after,
            },
            Self::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => EspaceStandardChangeKey::Erc1155TransferSingle {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                to: *to,
                token_id: *token_id,
                amount: *raw_amount,
            },
            Self::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            } => EspaceStandardChangeKey::Erc1155MintSingle {
                contract: *contract_address,
                operator: *operator,
                to: *to,
                token_id: *token_id,
                amount: *raw_amount,
            },
            Self::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            } => EspaceStandardChangeKey::Erc1155BurnSingle {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                token_id: *token_id,
                amount: *raw_amount,
            },
            Self::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => EspaceStandardChangeKey::Erc1155TransferBatch {
                contract: *contract_address,
                operator: *operator,
                from: *from,
                to: *to,
                items: items
                    .iter()
                    .map(|item| (item.token_id, item.raw_amount))
                    .collect(),
            },
            Self::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items,
            } => EspaceStandardChangeKey::Erc1155MintBatch {
                contract: *contract_address,
                operator: *operator,
                to: *to,
                items: items
                    .iter()
                    .map(|item| (item.token_id, item.raw_amount))
                    .collect(),
            },
            Self::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items,
            } => EspaceStandardChangeKey::Erc1155BurnBatch {
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

impl EspaceChangeEntry {
    fn merge_duplicate(&mut self, other: Self) -> bool {
        // Metadata is optional presentation data.  A duplicate semantic fact
        // is therefore idempotent even when one component learned more fields.
        match (&mut self.change, other.change) {
            (
                EspaceStateChange::WrappedNativeDeposit(existing),
                EspaceStateChange::WrappedNativeDeposit(incoming),
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
                EspaceStateChange::WrappedNativeWithdrawal(existing),
                EspaceStateChange::WrappedNativeWithdrawal(incoming),
            ) => {
                merge_erc20_metadata(
                    &mut existing.metadata,
                    incoming.metadata,
                    &mut self.metadata_conflicts,
                    other.metadata_conflicts,
                );
                true
            }
            (EspaceStateChange::Standard(existing), EspaceStateChange::Standard(incoming)) => {
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

fn merge_erc20_metadata(
    existing: &mut Erc20Metadata,
    incoming: Erc20Metadata,
    conflicts: &mut EspaceMetadataConflicts,
    incoming_conflicts: EspaceMetadataConflicts,
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

fn merge_standard_metadata(
    existing: &mut EspaceStandardChange,
    incoming: EspaceStandardChange,
    conflicts: &mut EspaceMetadataConflicts,
    incoming_conflicts: EspaceMetadataConflicts,
) -> bool {
    match (existing, incoming) {
        (
            EspaceStandardChange::Erc20Transfer { metadata, .. }
            | EspaceStandardChange::Erc20Mint { metadata, .. }
            | EspaceStandardChange::Erc20Burn { metadata, .. },
            EspaceStandardChange::Erc20Transfer {
                metadata: incoming, ..
            }
            | EspaceStandardChange::Erc20Mint {
                metadata: incoming, ..
            }
            | EspaceStandardChange::Erc20Burn {
                metadata: incoming, ..
            },
        ) => {
            merge_erc20_metadata(metadata, incoming, conflicts, incoming_conflicts);
            true
        }
        (
            EspaceStandardChange::Erc20Approval { metadata, .. },
            EspaceStandardChange::Erc20Approval {
                metadata: incoming, ..
            },
        ) => {
            merge_erc20_metadata(metadata, incoming, conflicts, incoming_conflicts);
            true
        }
        (
            EspaceStandardChange::Erc721Transfer { metadata, .. }
            | EspaceStandardChange::Erc721Mint { metadata, .. }
            | EspaceStandardChange::Erc721Burn { metadata, .. },
            EspaceStandardChange::Erc721Transfer {
                metadata: incoming, ..
            }
            | EspaceStandardChange::Erc721Mint {
                metadata: incoming, ..
            }
            | EspaceStandardChange::Erc721Burn {
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
            EspaceStandardChange::Erc721Approval { metadata, .. },
            EspaceStandardChange::Erc721Approval {
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

#[derive(Debug, Clone)]
pub struct EspaceNativeAssetChangeRules {
    currency: EspaceNativeCurrency,
}

impl EspaceNativeAssetChangeRules {
    pub fn new(currency: EspaceNativeCurrency) -> Self {
        Self { currency }
    }
}

impl EspaceChangeRules for EspaceNativeAssetChangeRules {
    fn required_observations(&self) -> EspaceObservationRequirements {
        EspaceObservationRequirements::new()
    }

    fn derive_changes(
        &self,
        execution: &EspaceExecutedTransaction,
        state: &EspaceStateAccess,
    ) -> Result<EspaceChangeSet, EspaceChangeDerivationError> {
        let occurrences = native::derive_changes(execution, state, &self.currency)
            .map_err(EspaceChangeDerivationError::Existing)?;
        let mut builder = EspaceChangeSetBuilder::new();
        for occurrence in occurrences {
            let (position, change) = occurrence.into_parts();
            let position = EspaceExecutionPosition::from_index(position);
            match change {
                EspaceChange::NativeTransfer {
                    from,
                    to,
                    raw_amount,
                    currency,
                } => builder.native_transfer(position, from, to, raw_amount, currency)?,
                EspaceChange::SelfDestructBurn {
                    contract_address,
                    raw_amount,
                    currency,
                } => builder.selfdestruct_burn(position, contract_address, raw_amount, currency)?,
                _ => {
                    return Err(EspaceChangeDerivationError::Conflict {
                        details: "native rules produced a non-native change".to_owned(),
                    });
                }
            }
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EspaceTokenChangeRules {
    wrapped_native_token: Address,
}

impl EspaceTokenChangeRules {
    pub const fn new(wrapped_native_token: Address) -> Self {
        Self {
            wrapped_native_token,
        }
    }
}

impl EspaceChangeRules for EspaceTokenChangeRules {
    fn required_observations(&self) -> EspaceObservationRequirements {
        let mut requirements = EspaceObservationRequirements::new();
        for topic0 in contract_standards::supported_event_topics() {
            requirements.checkpoint_any_address(*topic0);
        }
        for checkpoint in wrapped_native::log_checkpoints(self.wrapped_native_token) {
            requirements.insert(EspaceLogCheckpoint {
                address: checkpoint
                    .address
                    .map(|address| Address::from_slice(address.as_bytes())),
                topic0: alloy_primitives::B256::from_slice(checkpoint.topic0.as_bytes()),
            });
        }
        requirements
    }

    fn derive_changes(
        &self,
        execution: &EspaceExecutedTransaction,
        state: &EspaceStateAccess,
    ) -> Result<EspaceChangeSet, EspaceChangeDerivationError> {
        if !execution.is_success() {
            return Ok(EspaceChangeSet::default());
        }
        let occurrences =
            standards::derive_verified_changes(execution, state, self.wrapped_native_token)
                .map_err(EspaceChangeDerivationError::Existing)?;
        let mut builder = EspaceChangeSetBuilder::new();
        for occurrence in occurrences {
            match occurrence {
                standards::VerifiedChange::Standard { position, change } => {
                    builder.standard(EspaceExecutionPosition::from_index(position), change)?
                }
                standards::VerifiedChange::Wrapped {
                    position,
                    contract,
                    account,
                    amount,
                    direction,
                    metadata,
                } => match direction {
                    standards::WrappedOperation::Deposit => builder.wrapped_native_deposit(
                        EspaceExecutionPosition::from_index(position),
                        contract,
                        account,
                        amount,
                        metadata,
                    )?,
                    standards::WrappedOperation::Withdrawal => builder.wrapped_native_withdrawal(
                        EspaceExecutionPosition::from_index(position),
                        contract,
                        account,
                        amount,
                        metadata,
                    )?,
                },
            }
        }
        Ok(builder.finish())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EspaceAccountDelegationChangeRules;

impl EspaceChangeRules for EspaceAccountDelegationChangeRules {
    fn required_observations(&self) -> EspaceObservationRequirements {
        EspaceObservationRequirements::new()
    }

    fn derive_changes(
        &self,
        execution: &EspaceExecutedTransaction,
        state: &EspaceStateAccess,
    ) -> Result<EspaceChangeSet, EspaceChangeDerivationError> {
        let mut authorizations = BTreeMap::<Address, Vec<_>>::new();
        for authorization in execution.applied_authorizations() {
            authorizations
                .entry(authorization.account())
                .or_default()
                .push(authorization);
        }

        let mut builder = EspaceChangeSetBuilder::new();
        for (account, authorizations) in authorizations {
            let before_account = state
                .initial()
                .read_account(account)
                .map_err(|error| EspaceChangeDerivationError::Existing(error.into()))?;
            let after_account = state
                .finalized()
                .read_account(account)
                .map_err(|error| EspaceChangeDerivationError::Existing(error.into()))?;
            let before = delegation_state(account, &before_account)?;
            let after = delegation_state(account, &after_account)?;

            // The executor increments the transaction sender nonce before it
            // processes the EIP-7702 authorization list.  That increment is
            // part of the authority's nonce when the sender authorizes itself.
            let mut expected_nonce = before.nonce;
            if account == execution.transaction_sender() {
                expected_nonce = expected_nonce.checked_add(1).ok_or_else(|| {
                    EspaceChangeDerivationError::Conflict {
                        details: format!(
                            "transaction sender nonce overflow for delegation account {account}"
                        ),
                    }
                })?;
            }
            for authorization in &authorizations {
                if authorization.nonce() != expected_nonce {
                    return Err(EspaceChangeDerivationError::Conflict {
                        details: format!(
                            "successful authorization for {account} used nonce {}, expected {expected_nonce}",
                            authorization.nonce()
                        ),
                    });
                }
                expected_nonce = expected_nonce.checked_add(1).ok_or_else(|| {
                    EspaceChangeDerivationError::Conflict {
                        details: format!(
                            "successful authorization nonce overflow for account {account}"
                        ),
                    }
                })?;
            }

            let expected_delegate = authorizations.last().and_then(|authorization| {
                (authorization.delegate() != Address::ZERO).then_some(authorization.delegate())
            });
            if after.nonce != expected_nonce || after.delegate != expected_delegate {
                return Err(EspaceChangeDerivationError::Conflict {
                    details: format!(
                        "final delegation state for {account} does not match successful authorization results"
                    ),
                });
            }
            builder.account_delegation(account, before, after)?;
        }
        Ok(builder.finish())
    }
}

fn delegation_state(
    account: Address,
    state: &EspaceAccountState,
) -> Result<EspaceAccountDelegation, EspaceChangeDerivationError> {
    let nonce =
        u64::try_from(state.nonce()).map_err(|_| EspaceChangeDerivationError::Conflict {
            details: format!("eSpace nonce for delegation account {account} exceeds u64"),
        })?;
    Ok(EspaceAccountDelegation {
        delegate: state.delegation(),
        nonce,
    })
}

#[derive(Debug, Clone)]
pub struct DefaultEspaceChangeRules {
    components: CombinedEspaceChangeRules<
        CombinedEspaceChangeRules<EspaceNativeAssetChangeRules, EspaceAccountDelegationChangeRules>,
        EspaceTokenChangeRules,
    >,
}

impl DefaultEspaceChangeRules {
    pub fn new(currency: EspaceNativeCurrency, wrapped_native_token: Address) -> Self {
        Self {
            components: CombinedEspaceChangeRules::new(
                CombinedEspaceChangeRules::new(
                    EspaceNativeAssetChangeRules::new(currency),
                    EspaceAccountDelegationChangeRules,
                ),
                EspaceTokenChangeRules::new(wrapped_native_token),
            ),
        }
    }
}

impl EspaceChangeRules for DefaultEspaceChangeRules {
    fn required_observations(&self) -> EspaceObservationRequirements {
        self.components.required_observations()
    }

    fn derive_changes(
        &self,
        execution: &EspaceExecutedTransaction,
        state: &EspaceStateAccess,
    ) -> Result<EspaceChangeSet, EspaceChangeDerivationError> {
        self.components.derive_changes(execution, state)
    }
}

#[derive(Debug)]
pub(crate) struct ChangeOccurrence {
    position: usize,
    change: EspaceChange,
}

impl ChangeOccurrence {
    pub(crate) const fn new(position: usize, change: EspaceChange) -> Self {
        Self { position, change }
    }

    pub(crate) fn into_parts(self) -> (usize, EspaceChange) {
        (self.position, self.change)
    }
}

pub(crate) struct NestedEspaceEffects {
    standard_occurrences: Vec<DecodedStandardOccurrence>,
    wrapped_native_occurrences: Vec<WrappedNativeOccurrence>,
}

impl NestedEspaceEffects {
    pub(crate) fn from_trace(
        trace: &CommittedExecutionTrace,
        root_frame_ids: &[crate::execution::FrameId],
        wrapped_native_token: Address,
    ) -> Result<Self, EspaceChangesError> {
        let includes_frame = |frame_id| {
            root_frame_ids
                .iter()
                .any(|root_id| trace.frame_is_within(frame_id, *root_id))
        };
        Ok(Self {
            standard_occurrences: decode_standard_occurrences_in_scope(trace, includes_frame)?,
            wrapped_native_occurrences: decode_wrapped_native_occurrences_in_scope(
                trace,
                wrapped_native_token,
                includes_frame,
            ),
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.standard_occurrences.is_empty() && self.wrapped_native_occurrences.is_empty()
    }

    pub(crate) fn metadata_call_occurrences(&self) -> Vec<(usize, MetadataCall<Address>)> {
        collect_metadata_call_occurrences(
            &self.standard_occurrences,
            &self.wrapped_native_occurrences,
        )
    }

    pub(crate) fn into_changes(
        self,
        metadata: &contract_standards::MetadataValues<Address>,
    ) -> Vec<ChangeOccurrence> {
        let mut changes = Vec::new();
        for occurrence in self.wrapped_native_occurrences {
            let change_metadata = metadata
                .erc20_metadata(&occurrence.contract_address())
                .unwrap_or_else(|_| {
                    unreachable!("nested eSpace metadata collection records every outcome")
                });
            changes.push(occurrence.into_change(change_metadata));
        }
        for occurrence in self.standard_occurrences {
            let change = occurrence
                .decoded_log
                .into_change(metadata)
                .unwrap_or_else(|_| {
                    unreachable!("nested eSpace metadata collection records every outcome")
                });
            changes.push(ChangeOccurrence::new(
                occurrence.position,
                EspaceChange::Standard(change),
            ));
        }
        changes
    }
}

fn collect_metadata_call_occurrences(
    standard_occurrences: &[DecodedStandardOccurrence],
    wrapped_native_occurrences: &[WrappedNativeOccurrence],
) -> Vec<(usize, MetadataCall<Address>)> {
    let mut calls = Vec::new();

    for occurrence in standard_occurrences {
        calls.extend(
            metadata_calls(std::iter::once(&occurrence.decoded_log))
                .into_iter()
                .map(|call| (occurrence.position, call)),
        );
    }
    for occurrence in wrapped_native_occurrences {
        let position = occurrence.position();
        let contract_address = occurrence.contract_address();
        calls.extend([
            (position, MetadataCall::Name { contract_address }),
            (position, MetadataCall::Symbol { contract_address }),
            (position, MetadataCall::Decimals { contract_address }),
        ]);
    }

    calls.sort_by_key(|(position, _)| *position);
    calls
}
