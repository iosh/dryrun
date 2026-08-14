mod analysis;
mod codec;
mod collection;

pub(crate) use analysis::analyze_governance_changes;
pub(crate) use collection::GovernanceAnalysisInput;

use super::{ChangePosition, VoteAllocation};

#[derive(Debug, Clone)]
pub(super) struct VoteLogs {
    position: ChangePosition,
    events: Vec<VoteEvent>,
}

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

impl VoteEvent {
    const fn round(self) -> u64 {
        match self {
            Self::Revoke { round, .. } | Self::Vote { round, .. } => round,
        }
    }

    const fn voter(self) -> alloy_primitives::Address {
        match self {
            Self::Revoke { voter, .. } | Self::Vote { voter, .. } => voter,
        }
    }
}
