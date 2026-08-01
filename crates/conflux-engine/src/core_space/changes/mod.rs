mod cfx;

use alloy_primitives::{Address, U256};
use contract_standards::{Position, PositionedStandardChange};
use simulation_changes::Change;

pub(crate) use cfx::{
    collect_cfx_operations, determine_gas_fee_payer, read_cfx_state_snapshot, verify_cfx_changes,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoreSpaceChange {
    StandardOrNative(Change),
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
    },
    StakingBurn {
        account: Address,
        raw_amount: U256,
    },
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
            CoreSpaceChange::StandardOrNative(positioned.change.into()),
        )
    }
}

pub(crate) fn into_ordered_core_space_changes(
    mut positioned_changes: Vec<PositionedCoreSpaceChange>,
) -> Vec<CoreSpaceChange> {
    positioned_changes.sort_by_key(|positioned| positioned.position);
    positioned_changes
        .into_iter()
        .map(|positioned| positioned.change)
        .collect()
}
