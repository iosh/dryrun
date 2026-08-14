use std::collections::HashMap;

use cfx_types::Space;

use super::{VoteLogs, codec::decode_vote_event};
use crate::{
    core_space::{CoreSpaceChangesError, changes::ChangePosition},
    execution::{CommittedExecutionTrace, FrameId, TraceEvent},
};

#[derive(Debug)]
pub(crate) struct GovernanceAnalysisInput {
    groups: Vec<VoteLogs>,
}

impl GovernanceAnalysisInput {
    pub(crate) fn collect(trace: &CommittedExecutionTrace) -> Result<Self, CoreSpaceChangesError> {
        let params_contract =
            cfx_parameters::internal_contract_addresses::PARAMS_CONTROL_CONTRACT_ADDRESS;
        let mut group_by_frame = HashMap::<FrameId, usize>::new();
        let mut groups = Vec::<VoteLogs>::new();

        for event in trace.events() {
            let TraceEvent::Log {
                position,
                frame_id,
                address,
                topics,
                data,
            } = event
            else {
                continue;
            };
            if trace.frame(*frame_id).space != Space::Native || *address != params_contract {
                continue;
            }
            let Some(event) = decode_vote_event(topics, data)? else {
                continue;
            };

            let index = *group_by_frame.entry(*frame_id).or_insert_with(|| {
                groups.push(VoteLogs {
                    position: ChangePosition::new(*position, 0),
                    events: Vec::new(),
                });
                groups.len() - 1
            });
            groups[index].events.push(event);
        }

        Ok(Self { groups })
    }

    pub(super) fn groups(&self) -> &[VoteLogs] {
        &self.groups
    }
}
