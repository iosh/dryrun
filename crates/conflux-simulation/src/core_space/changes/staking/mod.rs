mod codec;
mod collection;
mod pos;
mod pos_state;
mod vote_lock;

pub(crate) use codec::decode_pos_staking_events;
pub(crate) use collection::{
    CommittedStakingCalls, StakingContractActivation, collect_committed_staking_calls,
};
pub(crate) use pos::verify_pos_staking_changes;
pub(crate) use pos_state::{PoSStateRequirements, read_pos_state_values};
pub(crate) use vote_lock::verify_vote_lock_changes;

use alloy_primitives::{Address, B256, U256};
use contract_standards::Position;

#[derive(Debug)]
pub(super) enum StakingCall {
    VoteLock {
        position: Position,
        account: Address,
        amount: U256,
        unlock_block_number: u64,
    },
    PoSRegistration {
        position: Position,
        account: Address,
        pos_identifier: B256,
        vote_count: u64,
    },
    PoSStakeIncrease {
        position: Position,
        account: Address,
        vote_count: u64,
    },
    PoSRetirementRequest {
        position: Position,
        account: Address,
        requested_vote_count: u64,
    },
}
