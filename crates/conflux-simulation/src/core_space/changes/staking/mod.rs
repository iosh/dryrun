mod codec;
mod collection;
mod pos;
mod pos_state;
mod verification;
mod vote_lock;

pub(crate) use collection::{ActiveContracts, CommittedCalls, collect_calls};
pub(crate) use pos::{PoSAnalysisInput, verify_pos_staking_changes};
pub(crate) use pos_state::{PoSStateReader, PoSStateValues};
pub(crate) use verification::analyze_balance_changes;
pub(crate) use vote_lock::verify_vote_lock_changes;

use crate::core_space::changes::ChangePosition;
use alloy_primitives::{Address, B256, U256};

#[derive(Debug, Clone, Copy)]
pub(crate) enum CommittedStakingCall {
    Deposit {
        position: ChangePosition,
        account: Address,
        amount: U256,
        transfer_position: usize,
    },
    Withdrawal {
        position: ChangePosition,
        account: Address,
        principal_amount: U256,
        reward_amount: U256,
        principal_transfer_position: usize,
        reward_transfer_position: usize,
    },
    VoteLock {
        position: ChangePosition,
        account: Address,
        required_locked_amount: U256,
        unlock_block_number: u64,
    },
}

impl CommittedStakingCall {
    pub(crate) const fn frame_position(self) -> usize {
        match self {
            Self::Deposit { position, .. }
            | Self::Withdrawal { position, .. }
            | Self::VoteLock { position, .. } => position.index,
        }
    }

    pub(crate) const fn account(self) -> Address {
        match self {
            Self::Deposit { account, .. }
            | Self::Withdrawal { account, .. }
            | Self::VoteLock { account, .. } => account,
        }
    }

    pub(crate) fn owned_transfer_positions(self) -> impl Iterator<Item = usize> {
        let positions = match self {
            Self::Deposit {
                transfer_position, ..
            } => [Some(transfer_position), None],
            Self::Withdrawal {
                principal_transfer_position,
                reward_transfer_position,
                ..
            } => [
                Some(principal_transfer_position),
                Some(reward_transfer_position),
            ],
            Self::VoteLock { .. } => [None, None],
        };
        positions.into_iter().flatten()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CommittedPoSCall {
    Registration {
        position: ChangePosition,
        account: Address,
        pos_identifier: B256,
        vote_count: u64,
    },
    StakeIncrease {
        position: ChangePosition,
        account: Address,
        vote_count: u64,
    },
    RetirementRequest {
        position: ChangePosition,
        account: Address,
        requested_vote_count: u64,
    },
}
