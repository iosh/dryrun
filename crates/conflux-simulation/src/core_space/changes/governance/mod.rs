mod analysis;
mod codec;
mod collection;

use super::VoteAllocation;
pub(crate) use analysis::derive_changes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VoteEvent {
    Revoke {
        round: u64,
        voter: alloy_primitives::Address,
        parameter: u16,
        allocation: VoteAllocation,
    },
    Vote {
        round: u64,
        voter: alloy_primitives::Address,
        parameter: u16,
        allocation: VoteAllocation,
    },
}
