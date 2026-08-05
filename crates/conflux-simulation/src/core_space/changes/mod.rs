mod cfx;
mod staking;

use alloy_primitives::{Address, B256, U256};
use contract_standards::{Position, PositionedStandardChange};
use simulation_changes::{Change, ChangeMetadata, NativeMetadata};

pub(crate) use cfx::{
    CfxBalanceLocation, CfxOperations, CfxStateValues, collect_cfx_operations,
    determine_gas_fee_payer, read_cfx_state_values, verify_cfx_changes,
};
pub(crate) use staking::{
    CommittedStakingCalls, PoSEvent, PoSStateRequirements, PoSStateValues,
    StakingContractActivation, collect_committed_staking_calls, decode_pos_staking_events,
    read_pos_state_values, verify_pos_staking_changes, verify_vote_lock_changes,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceChange {
    Asset(Change),
    StakingDeposit {
        account: Address,
        raw_amount: U256,
    },
    StakingWithdrawal {
        account: Address,
        raw_amount: U256,
        reward_raw_amount: U256,
    },
    NativeBurn {
        from: Address,
        raw_amount: U256,
        metadata: NativeMetadata,
    },
    StakingBurn {
        account: Address,
        raw_amount: U256,
    },
    StakingVoteLock {
        account: Address,
        unlock_block_number: u64,
        required_locked_raw_amount_before: U256,
        required_locked_raw_amount_after: U256,
    },
    PoSRegistration {
        account: Address,
        pos_identifier: B256,
        newly_locked_vote_count: u64,
        newly_locked_raw_amount: U256,
    },
    PoSStakeIncrease {
        account: Address,
        pos_identifier: B256,
        newly_locked_vote_count: u64,
        newly_locked_raw_amount: U256,
    },
    PoSRetirementRequest {
        account: Address,
        pos_identifier: B256,
        requested_vote_count: u64,
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
        configuration: SponsorshipConfiguration,
    },
    SponsorshipEligibilityRule {
        contract_address: Address,
        applies_to: SponsorshipEligibilityTarget,
        enabled_before: bool,
        enabled_after: bool,
    },
    StoragePointConversion {
        contract_address: Address,
        converted_cfx_raw_amount: U256,
    },
    CrossSpaceTransfer {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossSpaceAddress {
    CoreSpace(Address),
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
pub enum SponsorshipEligibilityTarget {
    Account(Address),
    AllAccounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositionedCoreSpaceChange {
    position: Position,
    change: CoreSpaceChange,
}

impl PositionedCoreSpaceChange {
    pub(crate) const fn new(position: Position, change: CoreSpaceChange) -> Self {
        Self { position, change }
    }
}

impl From<PositionedStandardChange> for PositionedCoreSpaceChange {
    fn from(positioned: PositionedStandardChange) -> Self {
        Self::new(
            positioned.position,
            CoreSpaceChange::Asset(positioned.change.into()),
        )
    }
}

pub(crate) fn order_and_enrich_core_space_changes(
    mut positioned_changes: Vec<PositionedCoreSpaceChange>,
    metadata: &ChangeMetadata,
) -> Vec<CoreSpaceChange> {
    positioned_changes.sort_by_key(|positioned| positioned.position);
    positioned_changes
        .into_iter()
        .map(|mut positioned| {
            if let CoreSpaceChange::Asset(change) = &mut positioned.change {
                metadata.enrich_change(change);
            } else if let CoreSpaceChange::NativeBurn {
                metadata: native_metadata,
                ..
            } = &mut positioned.change
            {
                *native_metadata = metadata.native_metadata().clone();
            }
            positioned.change
        })
        .collect()
}
