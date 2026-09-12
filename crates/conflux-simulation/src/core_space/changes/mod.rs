mod native_staking;

use std::{collections::BTreeMap, error::Error as StdError, sync::Arc};

use alloy_primitives::{Address, B256, Bytes, U256};
use conflux_provider::CoreAddress;
use contract_standards::StandardChange;

use super::{
    CoreSpaceChangesError, CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition,
    CoreSpaceStateAccess,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceNativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceChange {
    NativeTransfer {
        from: CoreAddress,
        to: CoreAddress,
        raw_amount: U256,
        currency: CoreSpaceNativeCurrency,
    },
    NativeBurn {
        from: CoreAddress,
        raw_amount: U256,
        currency: CoreSpaceNativeCurrency,
    },
    Standard(StandardChange<CoreAddress>),
    StakingDeposit {
        account: CoreAddress,
        amount: U256,
    },
    StakingWithdrawal {
        account: CoreAddress,
        principal_amount: U256,
        reward_amount: U256,
    },
    StakingVoteLock {
        account: CoreAddress,
        required_locked_amount: U256,
        unlock_block_number: u64,
    },
    PoSRegistration {
        account: CoreAddress,
        identifier: B256,
        bls_public_key: Bytes,
        vrf_public_key: Bytes,
        initial_vote_count: u64,
        locked_amount: U256,
    },
    PoSStakeIncrease {
        account: CoreAddress,
        identifier: B256,
        added_vote_count: u64,
        added_locked_amount: U256,
    },
    PoSRetirementRequest {
        account: CoreAddress,
        identifier: B256,
        requested_vote_count: u64,
    },
    GovernanceVoteCast {
        voter: CoreAddress,
        round: u64,
        votes: Vec<GovernanceVote>,
    },
    SponsorshipFunding {
        resource: SponsoredResource,
        contract_address: CoreAddress,
        sponsor: CoreAddress,
        contributed_amount: U256,
        pool_credited_amount: U256,
        terms: SponsorshipFundingTerms,
        replacement: Option<SponsorshipReplacement>,
    },
    ContractAdminSet {
        contract_address: CoreAddress,
        admin: Option<CoreAddress>,
    },
    SponsorshipAccessRuleSet {
        contract_address: CoreAddress,
        scope: SponsorshipAccessRuleScope,
        enabled: bool,
    },
    StoragePointConversion {
        contract_address: CoreAddress,
        from_sponsor_pool_amount: U256,
        from_storage_collateral_amount: U256,
    },
    CrossSpaceNativeTransfer {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    },
    Espace(crate::espace::EspaceChange),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoteAllocation {
    pub unchanged: U256,
    pub increase: U256,
    pub decrease: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceVote {
    pub parameter: GovernanceParameter,
    pub allocation: VoteAllocation,
    pub replaced_allocation: Option<VoteAllocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceParameter {
    PowBaseReward,
    PosRewardInterestRate,
    StoragePointProportion,
    BaseFeeShareProportion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossSpaceAddress {
    CoreSpace(CoreAddress),
    Espace(Address),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SponsoredResource {
    Gas,
    StorageCollateral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SponsorshipFundingTerms {
    Gas { gas_fee_upper_bound: U256 },
    StorageCollateral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SponsorshipReplacement {
    Gas {
        previous_sponsor: CoreAddress,
        pool_refunded_amount: U256,
    },
    StorageCollateral {
        previous_sponsor: CoreAddress,
        pool_refunded_amount: U256,
        collateral_compensation_amount: U256,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SponsorshipAccessRuleScope {
    Account(CoreAddress),
    AllAccounts,
}

/// A complete, position-ordered set of verified Core Space wallet semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreSpaceChangeSet {
    items: Vec<CoreSpaceChange>,
    entries: Vec<(CoreSpaceExecutionPosition, CoreSpaceChange)>,
}

impl CoreSpaceChangeSet {
    pub fn items(&self) -> &[CoreSpaceChange] {
        &self.items
    }

    pub fn into_items(self) -> Vec<CoreSpaceChange> {
        self.items
    }

    fn merge(self, other: Self) -> Result<Self, CoreSpaceChangeDerivationError> {
        let mut builder = CoreSpaceChangeSetBuilder::new();
        for (position, change) in self.entries.into_iter().chain(other.entries) {
            builder.insert(position, change)?;
        }
        Ok(builder.finish())
    }
}

/// Builder shared by built-in and custom Core Space change rules.
#[derive(Debug, Default)]
pub struct CoreSpaceChangeSetBuilder {
    entries: BTreeMap<CoreSpaceExecutionPosition, CoreSpaceChange>,
}

impl CoreSpaceChangeSetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(
        &mut self,
        position: CoreSpaceExecutionPosition,
        change: CoreSpaceChange,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if let Some(existing) = self.entries.get(&position) {
            if existing == &change {
                return Ok(());
            }
            return Err(CoreSpaceChangeDerivationError::Conflict {
                details: format!(
                    "multiple semantic changes were produced at Core Space execution position {}",
                    position.index()
                ),
            });
        }
        self.entries.insert(position, change);
        Ok(())
    }

    pub fn native_transfer(
        &mut self,
        position: CoreSpaceExecutionPosition,
        from: CoreAddress,
        to: CoreAddress,
        raw_amount: U256,
        currency: CoreSpaceNativeCurrency,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if raw_amount.is_zero() || from == to {
            return Ok(());
        }
        self.insert(
            position,
            CoreSpaceChange::NativeTransfer {
                from,
                to,
                raw_amount,
                currency,
            },
        )
    }

    pub fn native_burn(
        &mut self,
        position: CoreSpaceExecutionPosition,
        from: CoreAddress,
        raw_amount: U256,
        currency: CoreSpaceNativeCurrency,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if raw_amount.is_zero() {
            return Ok(());
        }
        self.insert(
            position,
            CoreSpaceChange::NativeBurn {
                from,
                raw_amount,
                currency,
            },
        )
    }

    pub fn staking_deposit(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        amount: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if amount.is_zero() {
            return Ok(());
        }
        self.insert(
            position,
            CoreSpaceChange::StakingDeposit { account, amount },
        )
    }

    pub fn staking_withdrawal(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        principal_amount: U256,
        reward_amount: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if principal_amount.is_zero() && reward_amount.is_zero() {
            return Ok(());
        }
        self.insert(
            position,
            CoreSpaceChange::StakingWithdrawal {
                account,
                principal_amount,
                reward_amount,
            },
        )
    }

    pub fn staking_vote_lock(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        required_locked_amount: U256,
        unlock_block_number: u64,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::StakingVoteLock {
                account,
                required_locked_amount,
                unlock_block_number,
            },
        )
    }

    pub fn finish(self) -> CoreSpaceChangeSet {
        let entries = self.entries.into_iter().collect::<Vec<_>>();
        let items = entries.iter().map(|(_, change)| change.clone()).collect();
        CoreSpaceChangeSet { items, entries }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoreSpaceChangeDerivationError {
    #[error(transparent)]
    Existing(#[from] CoreSpaceChangesError),
    #[error("Core Space change rules produced conflicting results: {details}")]
    Conflict { details: String },
    #[error("Core Space change rule `{rules}` failed: {source}")]
    RuleFailure {
        rules: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
}

impl CoreSpaceChangeDerivationError {
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

pub trait CoreSpaceChangeRules: Send + Sync + 'static {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError>;

    fn combine<R>(self, other: R) -> CombinedCoreSpaceChangeRules<Self, R>
    where
        Self: Sized,
        R: CoreSpaceChangeRules,
    {
        CombinedCoreSpaceChangeRules::new(self, other)
    }
}

#[derive(Debug, Clone)]
pub struct CombinedCoreSpaceChangeRules<A, B> {
    first: Arc<A>,
    second: Arc<B>,
}

impl<A, B> CombinedCoreSpaceChangeRules<A, B> {
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

impl<A, B> CoreSpaceChangeRules for CombinedCoreSpaceChangeRules<A, B>
where
    A: CoreSpaceChangeRules,
    B: CoreSpaceChangeRules,
{
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        let first = self.first.derive_changes(execution, state)?;
        let second = self.second.derive_changes(execution, state)?;
        first.merge(second)
    }
}

#[derive(Debug, Clone)]
pub struct CoreSpaceNativeAndStakingChangeRules {
    currency: CoreSpaceNativeCurrency,
}

impl CoreSpaceNativeAndStakingChangeRules {
    pub fn new(currency: CoreSpaceNativeCurrency) -> Self {
        Self { currency }
    }
}

impl CoreSpaceChangeRules for CoreSpaceNativeAndStakingChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        native_staking::derive_changes(execution, state, &self.currency).map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct DefaultCoreSpaceChangeRules {
    component: CoreSpaceNativeAndStakingChangeRules,
}

impl DefaultCoreSpaceChangeRules {
    pub fn new(currency: CoreSpaceNativeCurrency) -> Self {
        Self {
            component: CoreSpaceNativeAndStakingChangeRules::new(currency),
        }
    }
}

impl CoreSpaceChangeRules for DefaultCoreSpaceChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        self.component.derive_changes(execution, state)
    }
}
