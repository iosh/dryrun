use std::collections::BTreeMap;

use crate::primitive::u256_from_cfx;
use alloy_sol_types::{SolCall, sol};
use cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
use cfx_types::{Address, Space};
use cfx_vm_types::CallType;

use super::{
    CoreSpaceChangeSet, CoreSpaceChangeSetBuilder, SPONSORSHIP_POSITION_BASE, StoragePoints,
};
use crate::core_space::{
    CoreSpaceChangesError, CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition,
    CoreSpaceStateAccess,
};

sol! {
    interface SponsorCalls {
        function setSponsorForGas(address contract_address, uint256 upper_bound) external payable;
        function setSponsorForCollateral(address contract_address) external payable;
    }
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    position: usize,
}

pub(super) fn derive_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
) -> Result<CoreSpaceChangeSet, CoreSpaceChangesError> {
    let mut candidates = BTreeMap::<Address, Candidate>::new();
    let mut max_position = 0usize;

    for event in execution.trace().events() {
        max_position = max_position.max(event.position());
        match event {
            crate::execution::TraceEvent::StorageWrite {
                address, position, ..
            } if address.space == Space::Native => {
                candidates.entry(address.address).or_insert(Candidate {
                    position: *position,
                });
            }
            crate::execution::TraceEvent::FrameStart { position, frame_id } => {
                let Some(frame) = execution.trace().try_frame(*frame_id) else {
                    return Err(CoreSpaceChangesError::inconsistent_execution(
                        "sponsorship candidate references a missing frame",
                    ));
                };
                if frame.space != Space::Native {
                    continue;
                }
                if let crate::execution::FrameAction::Call {
                    call_type,
                    target,
                    code_address,
                    calldata,
                    ..
                } = &frame.action
                {
                    let sponsor_contract = SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
                    if *call_type == CallType::Call
                        && *target == sponsor_contract
                        && *code_address == sponsor_contract
                    {
                        if let Some(contract) = decode_sponsored_contract(calldata)? {
                            candidates.entry(contract).or_insert(Candidate {
                                position: *position,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for change in execution
        .storage_collateralized_entries
        .iter()
        .chain(&execution.storage_released)
    {
        candidates.entry(change.address).or_insert(Candidate {
            position: max_position,
        });
    }
    if (execution.gas_sponsor_paid || execution.storage_sponsor_paid)
        && let Some(recipient) = execution.transaction_recipient
    {
        candidates
            .entry(recipient)
            .or_insert(Candidate { position: 0 });
    }

    let mut builder = CoreSpaceChangeSetBuilder::new();
    // Sponsorship is a net state projection, so it has no one-to-one execution
    // position. Keep its synthetic range disjoint from other protocol resolvers.
    let mut next_position = SPONSORSHIP_POSITION_BASE;
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by_key(|(contract, candidate)| (candidate.position, *contract));
    for (contract, _) in candidates {
        let initial = state.initial().sponsorship(contract).map_err(|source| {
            CoreSpaceChangesError::state_access("read initial sponsorship state", source)
        })?;
        let finalized = state.finalized().sponsorship(contract).map_err(|source| {
            CoreSpaceChangesError::state_access("read finalized sponsorship state", source)
        })?;
        let contract_address = state.finalized().core_address(contract).map_err(|source| {
            CoreSpaceChangesError::state_access("convert sponsorship contract address", source)
        })?;
        if initial.gas_sponsor != finalized.gas_sponsor
            || initial.gas_balance != finalized.gas_balance
            || initial.gas_bound != finalized.gas_bound
        {
            let sponsor = finalized
                .gas_sponsor
                .map(|address| state.finalized().core_address(address))
                .transpose()
                .map_err(|source| {
                    CoreSpaceChangesError::state_access("convert gas sponsor address", source)
                })?;
            builder
                .gas_sponsorship(
                    CoreSpaceExecutionPosition::from_index(next_position),
                    contract_address,
                    sponsor,
                    u256_from_cfx(finalized.gas_balance),
                    u256_from_cfx(finalized.gas_bound),
                )
                .map_err(|error| {
                    CoreSpaceChangesError::inconsistent_execution(error.to_string())
                })?;
            next_position = next_position.saturating_add(1);
        }
        if initial.storage_sponsor != finalized.storage_sponsor
            || initial.storage_balance != finalized.storage_balance
            || initial.storage_points != finalized.storage_points
        {
            let sponsor = finalized
                .storage_sponsor
                .map(|address| state.finalized().core_address(address))
                .transpose()
                .map_err(|source| {
                    CoreSpaceChangesError::state_access("convert storage sponsor address", source)
                })?;
            builder
                .storage_sponsorship(
                    CoreSpaceExecutionPosition::from_index(next_position),
                    contract_address,
                    sponsor,
                    u256_from_cfx(finalized.storage_balance),
                    finalized.storage_points.map(|points| StoragePoints {
                        unused: u256_from_cfx(points.unused),
                        used: u256_from_cfx(points.used),
                    }),
                )
                .map_err(|error| {
                    CoreSpaceChangesError::inconsistent_execution(error.to_string())
                })?;
            next_position = next_position.saturating_add(1);
        }
        if initial.storage_collateral != finalized.storage_collateral {
            builder
                .storage_collateral(
                    CoreSpaceExecutionPosition::from_index(next_position),
                    contract_address,
                    u256_from_cfx(finalized.storage_collateral),
                )
                .map_err(|error| {
                    CoreSpaceChangesError::inconsistent_execution(error.to_string())
                })?;
            next_position = next_position.saturating_add(1);
        }
    }
    Ok(builder.finish())
}

fn decode_sponsored_contract(calldata: &[u8]) -> Result<Option<Address>, CoreSpaceChangesError> {
    if calldata.len() < 36 {
        return Ok(None);
    }
    let selector = &calldata[..4];
    let address = if selector == SponsorCalls::setSponsorForGasCall::SELECTOR {
        SponsorCalls::setSponsorForGasCall::abi_decode_validate(calldata)
            .map(|call| call.contract_address)
            .map_err(|error| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "invalid setSponsorForGas calldata: {error}"
                ))
            })?
    } else if selector == SponsorCalls::setSponsorForCollateralCall::SELECTOR {
        SponsorCalls::setSponsorForCollateralCall::abi_decode_validate(calldata)
            .map(|call| call.contract_address)
            .map_err(|error| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "invalid setSponsorForCollateral calldata: {error}"
                ))
            })?
    } else {
        return Ok(None);
    };
    Ok(Some(Address::from_slice(address.as_slice())))
}

#[cfg(test)]
mod tests {
    use super::{SponsorCalls, decode_sponsored_contract};
    use alloy_sol_types::SolCall;
    use cfx_types::Address;

    #[test]
    fn decodes_only_canonical_sponsor_calls() {
        let contract = Address::from_low_u64_be(7);
        let call = SponsorCalls::setSponsorForGasCall {
            contract_address: alloy_primitives::Address::from_slice(&contract.0),
            upper_bound: alloy_primitives::U256::from(1),
        };
        assert_eq!(
            decode_sponsored_contract(&call.abi_encode()).unwrap(),
            Some(contract)
        );

        let mut malformed = call.abi_encode();
        malformed[4] = 1;
        assert!(decode_sponsored_contract(&malformed).is_err());
        assert_eq!(decode_sponsored_contract(&malformed[..20]).unwrap(), None);
    }
}
