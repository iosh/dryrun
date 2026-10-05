use super::{Address, GovernanceVote, ProtocolChange, contract_callers};
use crate::{Error, StateError, address::VmAddress, view::View};
use alloy_sol_types::{SolCall, sol};
use cfx_parameters::internal_contract_addresses::PARAMS_CONTROL_CONTRACT_ADDRESS;
use cfx_types::AddressSpaceUtil;
use conflux_provider::Network;
use simulation_core::{CallResult, ExecutionTrace, StateView};

sol! {
    struct Vote { uint16 index; uint256[3] votes; }
    function readVote(address voter) external view returns (Vote[]);
}

pub(super) fn derive(
    trace: &ExecutionTrace<Address>,
    before: &View<'_>,
    after: &View<'_>,
    network: Network,
) -> Result<Vec<ProtocolChange>, Error> {
    let contract = Address::from_vm(PARAMS_CONTROL_CONTRACT_ADDRESS.with_native_space(), network);
    let mut changes = Vec::new();
    for voter in contract_callers(trace, contract)? {
        let old = read_votes(before, contract, voter)?;
        let new = read_votes(after, contract, voter)?;
        if old != new {
            changes.push(ProtocolChange::GovernanceVotes {
                voter,
                before: old,
                after: new,
            });
        }
    }
    Ok(changes)
}

fn read_votes(
    view: &View<'_>,
    contract: Address,
    voter: Address,
) -> Result<Vec<GovernanceVote>, Error> {
    let input = readVoteCall { voter: voter.raw() };
    let CallResult::Success(output) = view.call(contract, input.abi_encode().into())? else {
        return Err(StateError::Unavailable(format!(
            "protocol getter {} failed",
            readVoteCall::SIGNATURE
        ))
        .into());
    };
    let votes = readVoteCall::abi_decode_returns_validate(&output).map_err(|_| {
        StateError::Unavailable(format!(
            "protocol getter {} returned invalid data",
            readVoteCall::SIGNATURE
        ))
    })?;
    Ok(votes
        .into_iter()
        .map(|vote| GovernanceVote {
            index: vote.index,
            votes: vote.votes,
        })
        .collect())
}
