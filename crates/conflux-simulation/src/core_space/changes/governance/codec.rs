use alloy_sol_types::{SolCall, SolEvent, sol};

use super::VoteEvent;
use crate::{
    core_space::{CoreSpaceProtocolError, VoteAllocation},
    primitive::b256_from_cfx,
};

sol! {
    interface ParamsControl {
        struct VoteInput {
            uint16 index;
            uint256[3] votes;
        }
        function castVote(uint64 version, VoteInput[] votes);
    }
    event Vote(
        uint64 indexed round,
        address indexed voter,
        uint16 indexed parameter,
        uint256[3] allocation
    );
    event Revoke(
        uint64 indexed round,
        address indexed voter,
        uint16 indexed parameter,
        uint256[3] allocation
    );
}

#[derive(Debug, Clone)]
pub(super) struct CastVoteCall {
    pub(super) round: u64,
    pub(super) votes: Vec<(u16, VoteAllocation)>,
}

pub(super) fn decode_cast_vote(
    data: &[u8],
) -> Result<Option<CastVoteCall>, CoreSpaceProtocolError> {
    let Some(selector) = data.get(..4) else {
        return Ok(None);
    };
    if selector != ParamsControl::castVoteCall::SELECTOR {
        return Ok(None);
    }
    let call = ParamsControl::castVoteCall::abi_decode_validate(data).map_err(|error| {
        CoreSpaceProtocolError::inconsistent_execution(format!(
            "Core Space governance castVote call has invalid ABI data: {error}"
        ))
    })?;
    Ok(Some(CastVoteCall {
        round: call.version,
        votes: call
            .votes
            .into_iter()
            .map(|vote| {
                (
                    vote.index,
                    VoteAllocation {
                        unchanged: vote.votes[0],
                        increase: vote.votes[1],
                        decrease: vote.votes[2],
                    },
                )
            })
            .collect(),
    }))
}

pub(super) fn decode_vote_event(
    topics: &[cfx_types::H256],
    data: &[u8],
) -> Result<Option<VoteEvent>, CoreSpaceProtocolError> {
    let Some(signature) = topics.first().copied().map(b256_from_cfx) else {
        return Ok(None);
    };
    if signature == Vote::SIGNATURE_HASH {
        let event = Vote::decode_raw_log_validate(topics.iter().copied().map(b256_from_cfx), data)
            .map_err(|error| event_decode_error("Vote", error))?;
        Ok(Some(VoteEvent::Vote {
            round: event.round,
            voter: event.voter,
            parameter: event.parameter,
            allocation: allocation(event.allocation),
        }))
    } else if signature == Revoke::SIGNATURE_HASH {
        let event =
            Revoke::decode_raw_log_validate(topics.iter().copied().map(b256_from_cfx), data)
                .map_err(|error| event_decode_error("Revoke", error))?;
        Ok(Some(VoteEvent::Revoke {
            round: event.round,
            voter: event.voter,
            parameter: event.parameter,
            allocation: allocation(event.allocation),
        }))
    } else {
        Ok(None)
    }
}

fn allocation(values: [alloy_primitives::U256; 3]) -> VoteAllocation {
    VoteAllocation {
        unchanged: values[0],
        increase: values[1],
        decrease: values[2],
    }
}

fn event_decode_error(name: &str, error: alloy_sol_types::Error) -> CoreSpaceProtocolError {
    CoreSpaceProtocolError::inconsistent_execution(format!(
        "Core Space governance {name} event has invalid ABI data: {error}"
    ))
}
