mod codec;
mod collection;
mod pos;
mod pos_state;
mod vote_lock;

pub(crate) use collection::{
    CommittedStakingCalls, StakingContractActivation, collect_committed_staking_calls,
};
pub(crate) use pos::{PoSAnalysisInput, verify_pos_staking_changes};
pub(crate) use pos_state::{PoSStateReader, PoSStateValues};
pub(crate) use vote_lock::verify_vote_lock_changes;

use alloy_primitives::{Address, B256, U256};
use contract_standards::legacy::Position;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommittedVoteLockCall {
    pub(super) position: Position,
    pub(super) account: Address,
    pub(super) amount: U256,
    pub(super) unlock_block_number: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CommittedPoSCall {
    Registration {
        position: Position,
        account: Address,
        pos_identifier: B256,
        vote_count: u64,
    },
    StakeIncrease {
        position: Position,
        account: Address,
        vote_count: u64,
    },
    RetirementRequest {
        position: Position,
        account: Address,
        requested_vote_count: u64,
    },
}
