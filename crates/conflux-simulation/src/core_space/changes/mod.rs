mod cfx;
mod governance;
mod staking;
mod standards;

use std::fmt;

use alloy_primitives::{Address, B256, Bytes, U256};
use conflux_provider::{CoreAddress, Network};
use contract_standards::{DecodedStandardLog, MetadataValues, StandardChange};

use crate::core_space::CoreSpaceChangesError;

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
    SponsorshipDeposit {
        sponsored_resource: SponsoredResource,
        sponsor: CoreAddress,
        contract_address: CoreAddress,
        raw_amount: U256,
    },
    SponsorshipRefund {
        sponsored_resource: SponsoredResource,
        sponsor: CoreAddress,
        contract_address: CoreAddress,
        raw_amount: U256,
    },
    SponsorshipConfiguration {
        contract_address: CoreAddress,
        configuration: SponsorshipConfiguration,
    },
    SponsorshipEligibilityRule {
        contract_address: CoreAddress,
        applies_to: SponsorshipEligibilityTarget,
        enabled_before: bool,
        enabled_after: bool,
    },
    StoragePointConversion {
        contract_address: CoreAddress,
        converted_cfx_raw_amount: U256,
    },
    CrossSpaceTransfer {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SponsorshipConfiguration {
    Gas {
        sponsor_before: Option<CoreAddress>,
        sponsor_after: Option<CoreAddress>,
        max_sponsored_gas_fee_raw_amount_before: U256,
        max_sponsored_gas_fee_raw_amount_after: U256,
    },
    StorageCollateral {
        sponsor_before: Option<CoreAddress>,
        sponsor_after: Option<CoreAddress>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SponsorshipEligibilityTarget {
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
    SponsorshipDeposit {
        sponsored_resource: SponsoredResource,
        sponsor: Address,
        contract_address: Address,
        raw_amount: U256,
    },
    SponsorshipRefund {
        sponsored_resource: SponsoredResource,
        sponsor: Address,
        contract_address: Address,
        raw_amount: U256,
    },
    SponsorshipConfiguration {
        contract_address: Address,
        configuration: PendingSponsorshipConfiguration,
    },
    SponsorshipEligibilityRule {
        contract_address: Address,
        applies_to: PendingSponsorshipEligibilityTarget,
        enabled_before: bool,
        enabled_after: bool,
    },
    StoragePointConversion {
        contract_address: Address,
        converted_cfx_raw_amount: U256,
    },
    CrossSpaceTransfer {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingSponsorshipConfiguration {
    Gas {
        sponsor_before: Option<Address>,
        sponsor_after: Option<Address>,
        max_sponsored_gas_fee_raw_amount_before: U256,
        max_sponsored_gas_fee_raw_amount_after: U256,
    },
    StorageCollateral {
        sponsor_before: Option<Address>,
        sponsor_after: Option<Address>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PendingSponsorshipEligibilityTarget {
    Account(Address),
    AllAccounts,
}

#[derive(Debug)]
enum PendingChange {
    CoreSpace(PendingCoreSpaceChange),
    Standard(DecodedStandardLog<Address>),
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

    fn decoded_standard_log(&self) -> Option<&DecodedStandardLog<Address>> {
        match &self.change {
            PendingChange::Standard(change) => Some(change),
            PendingChange::CoreSpace(_) => None,
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
        PendingCoreSpaceChange::SponsorshipDeposit {
            sponsored_resource,
            sponsor,
            contract_address,
            raw_amount,
        } => CoreSpaceChange::SponsorshipDeposit {
            sponsored_resource,
            sponsor: address(sponsor)?,
            contract_address: address(contract_address)?,
            raw_amount,
        },
        PendingCoreSpaceChange::SponsorshipRefund {
            sponsored_resource,
            sponsor,
            contract_address,
            raw_amount,
        } => CoreSpaceChange::SponsorshipRefund {
            sponsored_resource,
            sponsor: address(sponsor)?,
            contract_address: address(contract_address)?,
            raw_amount,
        },
        PendingCoreSpaceChange::SponsorshipConfiguration {
            contract_address,
            configuration,
        } => CoreSpaceChange::SponsorshipConfiguration {
            contract_address: address(contract_address)?,
            configuration: resolve_sponsorship_configuration(configuration, network)?,
        },
        PendingCoreSpaceChange::SponsorshipEligibilityRule {
            contract_address,
            applies_to,
            enabled_before,
            enabled_after,
        } => CoreSpaceChange::SponsorshipEligibilityRule {
            contract_address: address(contract_address)?,
            applies_to: match applies_to {
                PendingSponsorshipEligibilityTarget::Account(account) => {
                    SponsorshipEligibilityTarget::Account(address(account)?)
                }
                PendingSponsorshipEligibilityTarget::AllAccounts => {
                    SponsorshipEligibilityTarget::AllAccounts
                }
            },
            enabled_before,
            enabled_after,
        },
        PendingCoreSpaceChange::StoragePointConversion {
            contract_address,
            converted_cfx_raw_amount,
        } => CoreSpaceChange::StoragePointConversion {
            contract_address: address(contract_address)?,
            converted_cfx_raw_amount,
        },
        PendingCoreSpaceChange::CrossSpaceTransfer {
            from,
            to,
            raw_amount,
        } => CoreSpaceChange::CrossSpaceTransfer {
            from: resolve_cross_space_address(from, network)?,
            to: resolve_cross_space_address(to, network)?,
            raw_amount,
        },
    })
}

fn resolve_sponsorship_configuration(
    configuration: PendingSponsorshipConfiguration,
    network: Network,
) -> Result<SponsorshipConfiguration, CoreSpaceChangesError> {
    Ok(match configuration {
        PendingSponsorshipConfiguration::Gas {
            sponsor_before,
            sponsor_after,
            max_sponsored_gas_fee_raw_amount_before,
            max_sponsored_gas_fee_raw_amount_after,
        } => SponsorshipConfiguration::Gas {
            sponsor_before: sponsor_before
                .map(|address| core_address(address, network))
                .transpose()?,
            sponsor_after: sponsor_after
                .map(|address| core_address(address, network))
                .transpose()?,
            max_sponsored_gas_fee_raw_amount_before,
            max_sponsored_gas_fee_raw_amount_after,
        },
        PendingSponsorshipConfiguration::StorageCollateral {
            sponsor_before,
            sponsor_after,
        } => SponsorshipConfiguration::StorageCollateral {
            sponsor_before: sponsor_before
                .map(|address| core_address(address, network))
                .transpose()?,
            sponsor_after: sponsor_after
                .map(|address| core_address(address, network))
                .transpose()?,
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
