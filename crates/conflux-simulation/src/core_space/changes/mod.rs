use super::CoreSpaceAnalysisError;
mod access;
mod admin;
mod cross_space;
mod governance;
mod native_staking;
mod nested_espace;
mod pos;
mod sponsorship;

use contract_standards::MetadataStore;
use simulation_core::analysis::MergeChanges;
use simulation_core::changes::{AssetEffect, ChangePosition, OrderedChanges};

use alloy_primitives::{Address, B256, Bytes, U256};
use conflux_provider::CoreAddress;
use contract_standards::StandardChange;

use super::{CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceStateAccess};

pub type CoreSpaceNativeCurrency = simulation_core::changes::NativeCurrency;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum CoreSpaceChange {
    StakingDeposit {
        account: CoreAddress,
        #[cfg_attr(feature = "serde", serde(rename = "rawAmount"))]
        amount: U256,
    },
    StakingWithdrawal {
        account: CoreAddress,
        #[cfg_attr(feature = "serde", serde(rename = "principalRawAmount"))]
        principal_amount: U256,
        #[cfg_attr(feature = "serde", serde(rename = "rewardRawAmount"))]
        reward_amount: U256,
    },
    StakingVoteLock {
        account: CoreAddress,
        #[cfg_attr(feature = "serde", serde(rename = "requiredLockedRawAmount"))]
        required_locked_amount: U256,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        unlock_block_number: u64,
    },
    #[cfg_attr(feature = "serde", serde(rename = "posRegistration"))]
    PoSRegistration {
        account: CoreAddress,
        identifier: B256,
        bls_public_key: Bytes,
        vrf_public_key: Bytes,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        initial_vote_count: u64,
        #[cfg_attr(feature = "serde", serde(rename = "lockedRawAmount"))]
        locked_amount: U256,
    },
    #[cfg_attr(feature = "serde", serde(rename = "posStakeIncrease"))]
    PoSStakeIncrease {
        account: CoreAddress,
        identifier: B256,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        added_vote_count: u64,
        #[cfg_attr(feature = "serde", serde(rename = "addedLockedRawAmount"))]
        added_locked_amount: U256,
    },
    #[cfg_attr(feature = "serde", serde(rename = "posRetirementRequest"))]
    PoSRetirementRequest {
        account: CoreAddress,
        identifier: B256,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        requested_vote_count: u64,
    },
    GovernanceVoteCast {
        voter: CoreAddress,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        round: u64,
        votes: Vec<GovernanceVote>,
    },
    GasSponsorship {
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        #[cfg_attr(feature = "serde", serde(rename = "balanceRawAmount"))]
        balance: U256,
        #[cfg_attr(feature = "serde", serde(rename = "gasFeeUpperBoundRawAmount"))]
        gas_fee_upper_bound: U256,
    },
    StorageSponsorship {
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        #[cfg_attr(feature = "serde", serde(rename = "balanceRawAmount"))]
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
        #[cfg_attr(feature = "serde", serde(skip))]
        resource: SponsoredResource,
        contract_address: CoreAddress,
        sponsor: CoreAddress,
        #[cfg_attr(feature = "serde", serde(rename = "contributedRawAmount"))]
        contributed_amount: U256,
        #[cfg_attr(feature = "serde", serde(rename = "poolCreditedRawAmount"))]
        pool_credited_amount: U256,
        #[cfg_attr(feature = "serde", serde(flatten))]
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
        #[cfg_attr(feature = "serde", serde(rename = "fromSponsorPoolRawAmount"))]
        from_sponsor_pool_amount: U256,
        #[cfg_attr(feature = "serde", serde(rename = "fromStorageCollateralRawAmount"))]
        from_storage_collateral_amount: U256,
    },
    CrossSpaceNativeTransfer {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    },
    #[cfg_attr(feature = "serde", serde(untagged))]
    Asset(simulation_core::changes::AssetChange<CoreAddress>),
    #[cfg_attr(feature = "serde", serde(untagged))]
    Espace(crate::espace::EspaceChange),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct VoteAllocation {
    pub unchanged: U256,
    pub increase: U256,
    pub decrease: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct GovernanceVote {
    pub parameter: GovernanceParameter,
    pub allocation: VoteAllocation,
    pub replaced_allocation: Option<VoteAllocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StoragePoints {
    pub unused: U256,
    pub used: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ContractAdminState {
    pub admin: Option<CoreAddress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum GovernanceParameter {
    PowBaseReward,
    PosRewardInterestRate,
    StoragePointProportion,
    BaseFeeShareProportion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "space", content = "address", rename_all = "camelCase")
)]
pub enum CrossSpaceAddress {
    CoreSpace(CoreAddress),
    Espace(Address),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum SponsoredResource {
    Gas,
    StorageCollateral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "resource",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum SponsorshipFundingTerms {
    Gas { gas_fee_upper_bound: U256 },
    StorageCollateral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(untagged, rename_all_fields = "camelCase"))]
pub enum SponsorshipReplacement {
    Gas {
        previous_sponsor: CoreAddress,
        #[cfg_attr(feature = "serde", serde(rename = "poolRefundedRawAmount"))]
        pool_refunded_amount: U256,
    },
    StorageCollateral {
        previous_sponsor: CoreAddress,
        #[cfg_attr(feature = "serde", serde(rename = "poolRefundedRawAmount"))]
        pool_refunded_amount: U256,
        #[cfg_attr(feature = "serde", serde(rename = "collateralCompensationRawAmount"))]
        collateral_compensation_amount: U256,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", content = "address", rename_all = "camelCase")
)]
pub enum SponsorshipAccessRuleScope {
    Account(CoreAddress),
    AllAccounts,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ProtocolEffect {
    StakingDeposit,
    StakingWithdrawal,
    StakingVoteLock,
    PoSRegistration,
    PoSStakeIncrease,
    PoSRetirement,
    Governance,
    GasSponsorship,
    StorageSponsorship,
    StorageCollateral,
    ContractAdmin,
    ContractAdminSet,
    StoragePointConversion,
    SponsorshipFunding(SponsoredResource),
    Access(SponsorshipAccessRuleScope),
    AccessSet(SponsorshipAccessRuleScope),
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CoreEffect {
    Asset(AssetEffect<CoreAddress>),
    Protocol {
        account: CoreAddress,
        effect: ProtocolEffect,
    },
    CrossSpace {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
    },
    Espace(AssetEffect),
}
impl CoreSpaceChange {
    fn effect(&self) -> CoreEffect {
        let (account, effect) = match self {
            Self::Asset(change) => return CoreEffect::Asset(change.effect()),
            Self::CrossSpaceNativeTransfer { from, to, .. } => {
                return CoreEffect::CrossSpace {
                    from: *from,
                    to: *to,
                };
            }
            Self::Espace(change) => return CoreEffect::Espace(change.effect()),
            Self::StakingDeposit { account, .. } => (*account, ProtocolEffect::StakingDeposit),
            Self::StakingWithdrawal { account, .. } => {
                (*account, ProtocolEffect::StakingWithdrawal)
            }
            Self::StakingVoteLock { account, .. } => (*account, ProtocolEffect::StakingVoteLock),
            Self::PoSRegistration { account, .. } => (*account, ProtocolEffect::PoSRegistration),
            Self::PoSStakeIncrease { account, .. } => (*account, ProtocolEffect::PoSStakeIncrease),
            Self::PoSRetirementRequest { account, .. } => (*account, ProtocolEffect::PoSRetirement),
            Self::GovernanceVoteCast { voter, .. } => (*voter, ProtocolEffect::Governance),
            Self::GasSponsorship {
                contract_address, ..
            } => (*contract_address, ProtocolEffect::GasSponsorship),
            Self::StorageSponsorship {
                contract_address, ..
            } => (*contract_address, ProtocolEffect::StorageSponsorship),
            Self::StorageCollateral {
                contract_address, ..
            } => (*contract_address, ProtocolEffect::StorageCollateral),
            Self::ContractAdmin {
                contract_address, ..
            } => (*contract_address, ProtocolEffect::ContractAdmin),
            Self::ContractAdminSet {
                contract_address, ..
            } => (*contract_address, ProtocolEffect::ContractAdminSet),
            Self::StoragePointConversion {
                contract_address, ..
            } => (*contract_address, ProtocolEffect::StoragePointConversion),
            Self::SponsorshipFunding {
                contract_address,
                resource,
                ..
            } => (
                *contract_address,
                ProtocolEffect::SponsorshipFunding(*resource),
            ),
            Self::SponsorshipAccessRule {
                contract_address,
                scope,
                ..
            } => (*contract_address, ProtocolEffect::Access(*scope)),
            Self::SponsorshipAccessRuleSet {
                contract_address,
                scope,
                ..
            } => (*contract_address, ProtocolEffect::AccessSet(*scope)),
        };
        CoreEffect::Protocol { account, effect }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreSpaceChangeSet {
    changes: OrderedChanges<CoreEffect, CoreSpaceChange>,
    metadata: MetadataStore<CrossSpaceAddress>,
}
impl CoreSpaceChangeSet {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&mut self, position: impl Into<ChangePosition>, change: CoreSpaceChange) {
        self.changes
            .insert(position.into(), change.effect(), change);
    }
    pub fn metadata(&self) -> &MetadataStore<CrossSpaceAddress> {
        &self.metadata
    }

    pub(crate) fn load_metadata(
        &mut self,
        state: &CoreSpaceStateAccess,
    ) -> Result<(), CoreSpaceAnalysisError> {
        let assets = self.items().filter_map(|change| match change {
            CoreSpaceChange::Asset(change) => change
                .metadata_request()
                .map(|(address, kind)| (CrossSpaceAddress::CoreSpace(address), kind)),
            CoreSpaceChange::Espace(change) => change
                .metadata_request()
                .map(|(address, kind)| (CrossSpaceAddress::Espace(address), kind)),
            _ => None,
        });
        self.metadata =
            contract_standards::load_metadata(state.finalized(), assets).map_err(|source| {
                super::CoreSpaceProtocolError::state_access("read token metadata", source)
            })?;
        Ok(())
    }
    pub fn items(&self) -> impl ExactSizeIterator<Item = &CoreSpaceChange> {
        self.changes.items()
    }
    pub fn into_items(self) -> Vec<CoreSpaceChange> {
        self.changes.into_items()
    }
}
impl simulation_core::analysis::MergeChanges for CoreSpaceChangeSet {
    fn merge(self, other: Self) -> Self {
        Self {
            changes: self.changes.merge(other.changes),
            metadata: MetadataStore::default(),
        }
    }
}
impl From<CoreSpaceExecutionPosition> for ChangePosition {
    fn from(position: CoreSpaceExecutionPosition) -> Self {
        Self::Execution(position.index())
    }
}
#[derive(Debug, Default)]
pub struct CoreSpaceChangeSetBuilder {
    changes: CoreSpaceChangeSet,
}
impl CoreSpaceChangeSetBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    fn insert(&mut self, position: impl Into<ChangePosition>, change: CoreSpaceChange) {
        self.changes
            .changes
            .insert(position.into(), change.effect(), change);
    }
    pub fn standard(
        &mut self,
        position: CoreSpaceExecutionPosition,
        change: StandardChange<CoreAddress>,
    ) {
        self.insert(
            position,
            CoreSpaceChange::Asset(simulation_core::changes::AssetChange::Standard(change)),
        )
    }
    pub fn native_transfer(
        &mut self,
        position: CoreSpaceExecutionPosition,
        from: CoreAddress,
        to: CoreAddress,
        raw_amount: U256,
        currency: CoreSpaceNativeCurrency,
    ) {
        if raw_amount.is_zero() || from == to {
            return;
        }
        self.insert(
            position,
            CoreSpaceChange::Asset(simulation_core::changes::AssetChange::NativeTransfer(
                simulation_core::changes::NativeTransfer {
                    from,
                    to,
                    raw_amount,
                    currency,
                },
            )),
        )
    }

    pub fn native_burn(
        &mut self,
        position: CoreSpaceExecutionPosition,
        from: CoreAddress,
        raw_amount: U256,
        currency: CoreSpaceNativeCurrency,
    ) {
        if raw_amount.is_zero() {
            return;
        }
        self.insert(
            position,
            CoreSpaceChange::Asset(simulation_core::changes::AssetChange::SelfDestructBurn(
                simulation_core::changes::NativeBurn {
                    contract_address: from,
                    raw_amount,
                    currency,
                },
            )),
        )
    }

    pub fn staking_deposit(
        &mut self,
        position: CoreSpaceExecutionPosition,
        account: CoreAddress,
        amount: U256,
    ) {
        if amount.is_zero() {
            return;
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
    ) {
        if principal_amount.is_zero() && reward_amount.is_zero() {
            return;
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
    ) {
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
    ) {
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
    ) {
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
    ) {
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
    ) {
        if votes.is_empty() {
            return;
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
        position: impl Into<ChangePosition>,
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        balance: U256,
        gas_fee_upper_bound: U256,
    ) {
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
        position: impl Into<ChangePosition>,
        contract_address: CoreAddress,
        sponsor: Option<CoreAddress>,
        balance: U256,
        storage_points: Option<StoragePoints>,
    ) {
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
        position: impl Into<ChangePosition>,
        contract_address: CoreAddress,
        raw_amount: U256,
    ) {
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
        position: impl Into<ChangePosition>,
        contract_address: CoreAddress,
        state: Option<ContractAdminState>,
    ) {
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
        position: impl Into<ChangePosition>,
        contract_address: CoreAddress,
        scope: SponsorshipAccessRuleScope,
        enabled: bool,
    ) {
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
    ) {
        self.insert(position, CoreSpaceChange::Espace(change))
    }

    pub(crate) fn cross_space_transfer(
        &mut self,
        position: CoreSpaceExecutionPosition,
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    ) {
        if raw_amount.is_zero() {
            return;
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
        self.changes
    }
}

pub(crate) fn derive_protocol_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    currency: &CoreSpaceNativeCurrency,
    espace_currency: &crate::espace::EspaceNativeCurrency,
) -> Result<CoreSpaceChangeSet, CoreSpaceAnalysisError> {
    let cross_space = cross_space::collect_committed_espace_scopes(execution.trace())?;
    let changes = native_staking::derive_changes(execution, state, currency, &cross_space)?;
    let changes = changes.merge(pos::derive_changes(execution, state)?);
    let changes = changes.merge(governance::derive_changes(execution, state)?);
    let changes = changes.merge(sponsorship::derive_changes(execution, state)?);
    let changes = changes.merge(admin::derive_changes(execution, state)?);
    let changes = changes.merge(access::derive_changes(execution, state)?);
    Ok(changes.merge(nested_espace::derive_native_changes(
        execution,
        espace_currency,
        &cross_space,
    )?))
}
