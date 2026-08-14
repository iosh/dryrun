use alloy_sol_types::{SolEvent, sol};

use super::VoteEvent;
use crate::{
    core_space::{CoreSpaceChangesError, VoteAllocation},
    primitive::b256_from_cfx,
};

sol! {
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

pub(super) fn decode_vote_event(
    topics: &[cfx_types::H256],
    data: &[u8],
) -> Result<Option<VoteEvent>, CoreSpaceChangesError> {
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

fn event_decode_error(name: &str, error: alloy_sol_types::Error) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space governance {name} event is not canonical ABI data: {error}"
    ))
}
