mod cfx;
mod governance;
mod staking;
mod standards;

use std::fmt;

use alloy_primitives::{Address, B256, Bytes, U256};
use conflux_provider::{CoreAddress, Network};
use contract_standards::{DecodedStandardLog, MetadataValues, StandardChange};

use crate::{core_space::CoreSpaceChangesError, espace::EspaceChange};

pub(crate) use cfx::{CfxAnalysisInput, CfxStateValues};
pub(crate) use governance::{GovernanceAnalysisInput, analyze_governance_changes};
pub(crate) use staking::{
    ActiveContracts, CommittedCalls, PoSAnalysisInput, PoSStateReader, PoSStateValues,
    analyze_balance_changes, analyze_pos_changes, collect_calls, verify_vote_lock_changes,
};
pub(crate) use standards::{collect_standard_changes, load_standard_metadata};

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
    Espace(EspaceChange),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ChangePosition {
    index: usize,
    item_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatePhase {
    Before,
    After,
}

impl fmt::Display for StatePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Before => "before",
            Self::After => "after",
        })
    }
}

impl ChangePosition {
    pub(crate) const fn new(index: usize, item_index: usize) -> Self {
        Self { index, item_index }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingCoreSpaceChange {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
    },
    NativeBurn {
        from: Address,
        raw_amount: U256,
    },
    StakingDeposit {
        account: Address,
        amount: U256,
    },
    StakingWithdrawal {
        account: Address,
        principal_amount: U256,
        reward_amount: U256,
    },
    StakingVoteLock {
        account: Address,
        required_locked_amount: U256,
        unlock_block_number: u64,
    },
    PoSRegistration {
        account: Address,
        identifier: B256,
        bls_public_key: Bytes,
        vrf_public_key: Bytes,
        initial_vote_count: u64,
        locked_amount: U256,
    },
    PoSStakeIncrease {
        account: Address,
        identifier: B256,
        added_vote_count: u64,
        added_locked_amount: U256,
    },
    PoSRetirementRequest {
        account: Address,
        identifier: B256,
        requested_vote_count: u64,
    },
    GovernanceVoteCast {
        voter: Address,
        round: u64,
        votes: Vec<GovernanceVote>,
    },
    SponsorshipFunding {
        resource: SponsoredResource,
        contract_address: Address,
        sponsor: Address,
        contributed_amount: U256,
        pool_credited_amount: U256,
        terms: SponsorshipFundingTerms,
        replacement: Option<PendingSponsorshipReplacement>,
    },
    ContractAdminSet {
        contract_address: Address,
        admin: Option<Address>,
    },
    SponsorshipAccessRuleSet {
        contract_address: Address,
        scope: PendingSponsorshipAccessRuleScope,
        enabled: bool,
    },
    StoragePointConversion {
        contract_address: Address,
        from_sponsor_pool_amount: U256,
        from_storage_collateral_amount: U256,
    },
    CrossSpaceNativeTransfer {
        from: PendingCrossSpaceAddress,
        to: PendingCrossSpaceAddress,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingCrossSpaceAddress {
    CoreSpace(Address),
    Espace(Address),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingSponsorshipReplacement {
    Gas {
        previous_sponsor: Address,
        pool_refunded_amount: U256,
    },
    StorageCollateral {
        previous_sponsor: Address,
        pool_refunded_amount: U256,
        collateral_compensation_amount: U256,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PendingSponsorshipAccessRuleScope {
    Account(Address),
    AllAccounts,
}

#[derive(Debug)]
enum PendingChange {
    CoreSpace(PendingCoreSpaceChange),
    Standard(DecodedStandardLog<Address>),
    Espace(EspaceChange),
}

#[derive(Debug)]
pub(crate) struct PositionedCoreSpaceChange {
    position: ChangePosition,
    change: PendingChange,
}

impl PositionedCoreSpaceChange {
    pub(crate) const fn new(position: ChangePosition, change: PendingCoreSpaceChange) -> Self {
        Self {
            position,
            change: PendingChange::CoreSpace(change),
        }
    }

    pub(crate) const fn standard(
        position: ChangePosition,
        change: DecodedStandardLog<Address>,
    ) -> Self {
        Self {
            position,
            change: PendingChange::Standard(change),
        }
    }

    pub(crate) const fn espace(position: ChangePosition, change: EspaceChange) -> Self {
        Self {
            position,
            change: PendingChange::Espace(change),
        }
    }

    pub(crate) const fn position(&self) -> ChangePosition {
        self.position
    }

    fn decoded_standard_log(&self) -> Option<&DecodedStandardLog<Address>> {
        match &self.change {
            PendingChange::Standard(change) => Some(change),
            PendingChange::CoreSpace(_) | PendingChange::Espace(_) => None,
        }
    }
}

pub(crate) fn finish_core_space_changes(
    mut positioned_changes: Vec<PositionedCoreSpaceChange>,
    metadata: &MetadataValues<Address>,
    network: Network,
    currency: &CoreSpaceNativeCurrency,
) -> Result<Vec<CoreSpaceChange>, CoreSpaceChangesError> {
    positioned_changes.sort_by_key(|positioned| positioned.position);
    positioned_changes
        .into_iter()
        .map(|positioned| match positioned.change {
            PendingChange::CoreSpace(change) => resolve_change(change, network, currency),
            PendingChange::Standard(change) => {
                let change = change.into_change(metadata).map_err(|_| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "a decoded Core Space standard change is missing metadata",
                    )
                })?;
                Ok(CoreSpaceChange::Standard(resolve_standard_change(
                    change, network,
                )?))
            }
            PendingChange::Espace(change) => Ok(CoreSpaceChange::Espace(change)),
        })
        .collect()
}

fn resolve_change(
    change: PendingCoreSpaceChange,
    network: Network,
    currency: &CoreSpaceNativeCurrency,
) -> Result<CoreSpaceChange, CoreSpaceChangesError> {
    let address = |value| core_address(value, network);
    Ok(match change {
        PendingCoreSpaceChange::NativeTransfer {
            from,
            to,
            raw_amount,
        } => CoreSpaceChange::NativeTransfer {
            from: address(from)?,
            to: address(to)?,
            raw_amount,
            currency: currency.clone(),
        },
        PendingCoreSpaceChange::NativeBurn { from, raw_amount } => CoreSpaceChange::NativeBurn {
            from: address(from)?,
            raw_amount,
            currency: currency.clone(),
        },
        PendingCoreSpaceChange::StakingDeposit { account, amount } => {
            CoreSpaceChange::StakingDeposit {
                account: address(account)?,
                amount,
            }
        }
        PendingCoreSpaceChange::StakingWithdrawal {
            account,
            principal_amount,
            reward_amount,
        } => CoreSpaceChange::StakingWithdrawal {
            account: address(account)?,
            principal_amount,
            reward_amount,
        },
        PendingCoreSpaceChange::StakingVoteLock {
            account,
            required_locked_amount,
            unlock_block_number,
        } => CoreSpaceChange::StakingVoteLock {
            account: address(account)?,
            required_locked_amount,
            unlock_block_number,
        },
        PendingCoreSpaceChange::PoSRegistration {
            account,
            identifier,
            bls_public_key,
            vrf_public_key,
            initial_vote_count,
            locked_amount,
        } => CoreSpaceChange::PoSRegistration {
            account: address(account)?,
            identifier,
            bls_public_key,
            vrf_public_key,
            initial_vote_count,
            locked_amount,
        },
        PendingCoreSpaceChange::PoSStakeIncrease {
            account,
            identifier,
            added_vote_count,
            added_locked_amount,
        } => CoreSpaceChange::PoSStakeIncrease {
            account: address(account)?,
            identifier,
            added_vote_count,
            added_locked_amount,
        },
        PendingCoreSpaceChange::PoSRetirementRequest {
            account,
            identifier,
            requested_vote_count,
        } => CoreSpaceChange::PoSRetirementRequest {
            account: address(account)?,
            identifier,
            requested_vote_count,
        },
        PendingCoreSpaceChange::GovernanceVoteCast {
            voter,
            round,
            votes,
        } => CoreSpaceChange::GovernanceVoteCast {
            voter: address(voter)?,
            round,
            votes,
        },
        PendingCoreSpaceChange::SponsorshipFunding {
            resource,
            contract_address,
            sponsor,
            contributed_amount,
            pool_credited_amount,
            terms,
            replacement,
        } => CoreSpaceChange::SponsorshipFunding {
            resource,
            contract_address: address(contract_address)?,
            sponsor: address(sponsor)?,
            contributed_amount,
            pool_credited_amount,
            terms,
            replacement: replacement
                .map(|replacement| {
                    Ok(match replacement {
                        PendingSponsorshipReplacement::Gas {
                            previous_sponsor,
                            pool_refunded_amount,
                        } => SponsorshipReplacement::Gas {
                            previous_sponsor: address(previous_sponsor)?,
                            pool_refunded_amount,
                        },
                        PendingSponsorshipReplacement::StorageCollateral {
                            previous_sponsor,
                            pool_refunded_amount,
                            collateral_compensation_amount,
                        } => SponsorshipReplacement::StorageCollateral {
                            previous_sponsor: address(previous_sponsor)?,
                            pool_refunded_amount,
                            collateral_compensation_amount,
                        },
                    })
                })
                .transpose()?,
        },
        PendingCoreSpaceChange::ContractAdminSet {
            contract_address,
            admin,
        } => CoreSpaceChange::ContractAdminSet {
            contract_address: address(contract_address)?,
            admin: admin.map(address).transpose()?,
        },
        PendingCoreSpaceChange::SponsorshipAccessRuleSet {
            contract_address,
            scope,
            enabled,
        } => CoreSpaceChange::SponsorshipAccessRuleSet {
            contract_address: address(contract_address)?,
            scope: match scope {
                PendingSponsorshipAccessRuleScope::Account(account) => {
                    SponsorshipAccessRuleScope::Account(address(account)?)
                }
                PendingSponsorshipAccessRuleScope::AllAccounts => {
                    SponsorshipAccessRuleScope::AllAccounts
                }
            },
            enabled,
        },
        PendingCoreSpaceChange::StoragePointConversion {
            contract_address,
            from_sponsor_pool_amount,
            from_storage_collateral_amount,
        } => CoreSpaceChange::StoragePointConversion {
            contract_address: address(contract_address)?,
            from_sponsor_pool_amount,
            from_storage_collateral_amount,
        },
        PendingCoreSpaceChange::CrossSpaceNativeTransfer {
            from,
            to,
            raw_amount,
        } => CoreSpaceChange::CrossSpaceNativeTransfer {
            from: resolve_cross_space_address(from, network)?,
            to: resolve_cross_space_address(to, network)?,
            raw_amount,
        },
    })
}

fn resolve_cross_space_address(
    address: PendingCrossSpaceAddress,
    network: Network,
) -> Result<CrossSpaceAddress, CoreSpaceChangesError> {
    Ok(match address {
        PendingCrossSpaceAddress::CoreSpace(address) => {
            CrossSpaceAddress::CoreSpace(core_address(address, network)?)
        }
        PendingCrossSpaceAddress::Espace(address) => CrossSpaceAddress::Espace(address),
    })
}

fn resolve_standard_change(
    change: StandardChange<Address>,
    network: Network,
) -> Result<StandardChange<CoreAddress>, CoreSpaceChangesError> {
    let address = |value| core_address(value, network);
    Ok(match change {
        StandardChange::Erc20Transfer {
            contract_address,
            from,
            to,
            raw_amount,
            metadata,
        } => StandardChange::Erc20Transfer {
            contract_address: address(contract_address)?,
            from: address(from)?,
            to: address(to)?,
            raw_amount,
            metadata,
        },
        StandardChange::Erc20Approval {
            contract_address,
            owner,
            spender,
            approved_amount,
            metadata,
        } => StandardChange::Erc20Approval {
            contract_address: address(contract_address)?,
            owner: address(owner)?,
            spender: address(spender)?,
            approved_amount,
            metadata,
        },
        StandardChange::Erc721Transfer {
            contract_address,
            from,
            to,
            token_id,
            metadata,
        } => StandardChange::Erc721Transfer {
            contract_address: address(contract_address)?,
            from: address(from)?,
            to: address(to)?,
            token_id,
            metadata,
        },
        StandardChange::Erc721Approval {
            contract_address,
            owner,
            approved_address,
            token_id,
            metadata,
        } => StandardChange::Erc721Approval {
            contract_address: address(contract_address)?,
            owner: address(owner)?,
            approved_address: approved_address.map(address).transpose()?,
            token_id,
            metadata,
        },
        StandardChange::OperatorApproval {
            contract_address,
            owner,
            operator,
            approved,
        } => StandardChange::OperatorApproval {
            contract_address: address(contract_address)?,
            owner: address(owner)?,
            operator: address(operator)?,
            approved,
        },
        StandardChange::Erc1155TransferSingle {
            contract_address,
            operator,
            from,
            to,
            token_id,
            raw_amount,
        } => StandardChange::Erc1155TransferSingle {
            contract_address: address(contract_address)?,
            operator: address(operator)?,
            from: address(from)?,
            to: address(to)?,
            token_id,
            raw_amount,
        },
        StandardChange::Erc1155TransferBatch {
            contract_address,
            operator,
            from,
            to,
            items,
        } => StandardChange::Erc1155TransferBatch {
            contract_address: address(contract_address)?,
            operator: address(operator)?,
            from: address(from)?,
            to: address(to)?,
            items,
        },
    })
}

fn core_address(address: Address, network: Network) -> Result<CoreAddress, CoreSpaceChangesError> {
    CoreAddress::from_bytes(address.into_array(), network).map_err(|error| {
        CoreSpaceChangesError::inconsistent_execution(format!(
            "failed to represent a Core Space change address: {error}"
        ))
    })
}
