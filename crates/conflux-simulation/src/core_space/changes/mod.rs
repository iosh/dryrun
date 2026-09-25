mod access;
mod admin;
mod governance;
mod native_staking;
mod nested_espace;
mod pos;
mod sponsorship;

pub(super) const SPONSORSHIP_POSITION_BASE: usize = usize::MAX / 4;
pub(super) const ADMIN_POSITION_BASE: usize = usize::MAX / 2;
pub(super) const ACCESS_RULE_POSITION_BASE: usize = usize::MAX / 4 * 3;

use std::{collections::BTreeMap, error::Error as StdError, sync::Arc};

use alloy_primitives::{Address, B256, Bytes, U256};
use conflux_provider::CoreAddress;
use contract_standards::StandardChange;

use super::{
    CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceProtocolError,
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
    GasSponsorship {
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        balance: U256,
        gas_fee_upper_bound: U256,
    },
    StorageSponsorship {
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        balance: U256,
        storage_points: Option<StoragePoints>,
    },
    StorageCollateral {
        contract_address: CoreAddress,
        raw_amount: U256,
    },
    ContractAdmin {
        contract_address: CoreAddress,
        state: Option<ContractAdminState>,
    },
    SponsorshipAccessRule {
        contract_address: CoreAddress,
        scope: SponsorshipAccessRuleScope,
        enabled: bool,
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
pub struct StoragePoints {
    pub unused: U256,
    pub used: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractAdminState {
    pub admin: Option<CoreAddress>,
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

    #[expect(
        clippy::too_many_arguments,
        reason = "the builder mirrors the public PoS registration change payload"
    )]
    pub fn pos_registration(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        identifier: B256,
        bls_public_key: Bytes,
        vrf_public_key: Bytes,
        initial_vote_count: u64,
        locked_amount: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::PoSRegistration {
                account,
                identifier,
                bls_public_key,
                vrf_public_key,
                initial_vote_count,
                locked_amount,
            },
        )
    }

    pub fn pos_stake_increase(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        identifier: B256,
        added_vote_count: u64,
        added_locked_amount: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::PoSStakeIncrease {
                account,
                identifier,
                added_vote_count,
                added_locked_amount,
            },
        )
    }

    pub fn pos_retirement_request(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        identifier: B256,
        requested_vote_count: u64,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::PoSRetirementRequest {
                account,
                identifier,
                requested_vote_count,
            },
        )
    }

    pub fn governance_vote(
        &mut self,
        position: CoreSpaceExecutionPosition,
        voter: CoreAddress,
        round: u64,
        votes: Vec<GovernanceVote>,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if votes.is_empty() {
            return Ok(());
        }
        self.insert(
            position,
            CoreSpaceChange::GovernanceVoteCast {
                voter,
                round,
                votes,
            },
        )
    }

    pub fn gas_sponsorship(
        &mut self,
        position: CoreSpaceExecutionPosition,
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        balance: U256,
        gas_fee_upper_bound: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::GasSponsorship {
                contract_address,
                sponsor,
                balance,
                gas_fee_upper_bound,
            },
        )
    }

    pub fn storage_sponsorship(
        &mut self,
        position: CoreSpaceExecutionPosition,
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        balance: U256,
        storage_points: Option<StoragePoints>,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::StorageSponsorship {
                contract_address,
                sponsor,
                balance,
                storage_points,
            },
        )
    }

    pub fn storage_collateral(
        &mut self,
        position: CoreSpaceExecutionPosition,
        contract_address: CoreAddress,
        raw_amount: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::StorageCollateral {
                contract_address,
                raw_amount,
            },
        )
    }

    pub fn contract_admin(
        &mut self,
        position: CoreSpaceExecutionPosition,
        contract_address: CoreAddress,
        state: Option<ContractAdminState>,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::ContractAdmin {
                contract_address,
                state,
            },
        )
    }

    pub fn sponsorship_access_rule(
        &mut self,
        position: CoreSpaceExecutionPosition,
        contract_address: CoreAddress,
        scope: SponsorshipAccessRuleScope,
        enabled: bool,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(
            position,
            CoreSpaceChange::SponsorshipAccessRule {
                contract_address,
                scope,
                enabled,
            },
        )
    }

    pub(crate) fn espace(
        &mut self,
        position: CoreSpaceExecutionPosition,
        change: crate::espace::EspaceChange,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        self.insert(position, CoreSpaceChange::Espace(change))
    }

    pub(crate) fn cross_space_transfer(
        &mut self,
        position: CoreSpaceExecutionPosition,
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    ) -> Result<(), CoreSpaceChangeDerivationError> {
        if raw_amount.is_zero() {
            return Ok(());
        }
        self.insert(
            position,
            CoreSpaceChange::CrossSpaceNativeTransfer {
                from,
                to,
                raw_amount,
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
    Protocol(#[from] CoreSpaceProtocolError),
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

/// Rules run only after successful execution. The configured composition must
/// account for every supported effect or return an error; partial sets are not published.
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

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreSpacePoSChangeRules;

impl CoreSpacePoSChangeRules {
    pub const fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreSpaceGovernanceChangeRules;

impl CoreSpaceGovernanceChangeRules {
    pub const fn new() -> Self {
        Self
    }
}

impl CoreSpaceChangeRules for CoreSpaceGovernanceChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        governance::derive_changes(execution, state).map_err(Into::into)
    }
}

impl CoreSpaceChangeRules for CoreSpacePoSChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        pos::derive_changes(execution, state).map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct DefaultCoreSpaceChangeRules {
    native_and_staking: CoreSpaceNativeAndStakingChangeRules,
    pos: CoreSpacePoSChangeRules,
    governance: CoreSpaceGovernanceChangeRules,
    sponsorship: CoreSpaceSponsorshipChangeRules,
    admin: CoreSpaceContractChangeRules,
    access: CoreSpaceAccessRuleChangeRules,
    espace_native_currency: crate::espace::EspaceNativeCurrency,
    espace_wrapped_native_token: alloy_primitives::Address,
    espace_change_rules_enabled: bool,
}

impl DefaultCoreSpaceChangeRules {
    pub fn new(currency: CoreSpaceNativeCurrency) -> Self {
        let mut rules = Self::new_with_espace(
            currency,
            crate::espace::EspaceNativeCurrency {
                name: "eSpace native currency".to_owned(),
                symbol: "CFX".to_owned(),
                decimals: 18,
            },
            alloy_primitives::Address::ZERO,
        );
        rules.espace_change_rules_enabled = false;
        rules
    }

    pub fn new_with_espace(
        currency: CoreSpaceNativeCurrency,
        espace_native_currency: crate::espace::EspaceNativeCurrency,
        espace_wrapped_native_token: alloy_primitives::Address,
    ) -> Self {
        Self {
            native_and_staking: CoreSpaceNativeAndStakingChangeRules::new(currency),
            pos: CoreSpacePoSChangeRules,
            governance: CoreSpaceGovernanceChangeRules,
            sponsorship: CoreSpaceSponsorshipChangeRules,
            admin: CoreSpaceContractChangeRules,
            access: CoreSpaceAccessRuleChangeRules,
            espace_native_currency,
            espace_wrapped_native_token,
            espace_change_rules_enabled: true,
        }
    }
}

impl CoreSpaceChangeRules for DefaultCoreSpaceChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        let native_and_staking = self.native_and_staking.derive_changes(execution, state)?;
        let pos = self.pos.derive_changes(execution, state)?;
        let governance = self.governance.derive_changes(execution, state)?;
        let sponsorship = self.sponsorship.derive_changes(execution, state)?;
        let admin = self.admin.derive_changes(execution, state)?;
        let access = self.access.derive_changes(execution, state)?;
        let nested_espace = if execution.nested_espace_scope_roots().is_empty() {
            CoreSpaceChangeSet::default()
        } else if !self.espace_change_rules_enabled {
            return Err(CoreSpaceChangeDerivationError::Protocol(
                CoreSpaceProtocolError::unsupported_operation(
                    "nested eSpace changes require eSpace chain configuration",
                ),
            ));
        } else {
            nested_espace::derive_native_changes(
                execution,
                state,
                &self.espace_native_currency,
                self.espace_wrapped_native_token,
            )?
        };
        native_and_staking
            .merge(pos)?
            .merge(governance)?
            .merge(sponsorship)?
            .merge(admin)?
            .merge(access)?
            .merge(nested_espace)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreSpaceSponsorshipChangeRules;

impl CoreSpaceSponsorshipChangeRules {
    pub const fn new() -> Self {
        Self
    }
}

impl CoreSpaceChangeRules for CoreSpaceSponsorshipChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        sponsorship::derive_changes(execution, state).map_err(Into::into)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreSpaceContractChangeRules;

impl CoreSpaceContractChangeRules {
    pub const fn new() -> Self {
        Self
    }
}

impl CoreSpaceChangeRules for CoreSpaceContractChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        admin::derive_changes(execution, state).map_err(Into::into)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreSpaceAccessRuleChangeRules;

impl CoreSpaceAccessRuleChangeRules {
    pub const fn new() -> Self {
        Self
    }
}

impl CoreSpaceChangeRules for CoreSpaceAccessRuleChangeRules {
    fn derive_changes(
        &self,
        execution: &CoreSpaceExecutedTransaction,
        state: &CoreSpaceStateAccess,
    ) -> Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError> {
        access::derive_changes(execution, state).map_err(Into::into)
    }
}

pub(crate) fn check_contract_support(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
) -> Result<(), CoreSpaceChangeDerivationError> {
    use crate::execution::FrameAction;
    use cfx_parameters::internal_contract_addresses::*;
    use cfx_types::{AddressSpaceUtil, Space};

    for (_, frame) in execution.committed_trace.frames() {
        let FrameAction::Call {
            code_address,
            target,
            call_type,
            ..
        } = frame.action
        else {
            return Err(CoreSpaceProtocolError::unsupported_operation(
                "contract creation requires an implementation-specific analyzer",
            )
            .into());
        };
        if frame.space == Space::Native && execution.is_active_internal_contract(code_address) {
            let supported = [
                ADMIN_CONTROL_CONTRACT_ADDRESS,
                SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS,
                STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS,
                POS_REGISTER_CONTRACT_ADDRESS,
                CROSS_SPACE_CONTRACT_ADDRESS,
                PARAMS_CONTROL_CONTRACT_ADDRESS,
                CONTEXT_CONTRACT_ADDRESS,
            ]
            .contains(&code_address);
            if !supported
                || target != code_address
                || !matches!(
                    call_type,
                    cfx_vm_types::CallType::Call | cfx_vm_types::CallType::StaticCall
                )
            {
                return Err(CoreSpaceProtocolError::unsupported_operation(format!(
                    "unverified internal contract call at {code_address:?}"
                ))
                .into());
            }
            continue;
        }
        for reader in [state.initial(), state.finalized()] {
            let code = reader
                .code(code_address.with_space(frame.space))
                .map_err(|source| {
                    CoreSpaceProtocolError::state_access("check contract implementation", source)
                })?;
            if code.is_some_and(|code| !code.is_empty()) {
                return Err(CoreSpaceProtocolError::unsupported_operation(format!(
                    "no verified implementation scope for code at {code_address:?} in {:?}",
                    frame.space
                ))
                .into());
            }
        }
    }
    Ok(())
}
