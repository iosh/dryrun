use std::collections::BTreeMap;

use alloy_sol_types::{SolCall, sol};
use cfx_parameters::internal_contract_addresses::ADMIN_CONTROL_CONTRACT_ADDRESS;
use cfx_types::{Address, Space};
use cfx_vm_types::CallType;

use super::{
    ADMIN_POSITION_BASE, ContractAdminState, CoreSpaceChangeSet, CoreSpaceChangeSetBuilder,
};
use crate::core_space::{
    CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceProtocolError,
    CoreSpaceStateAccess,
};

sol! {
    interface AdminCalls {
        function setAdmin(address contract_address, address new_admin_address) external;
        function destroy(address contract_address) external;
    }
}

pub(super) fn derive_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
) -> Result<CoreSpaceChangeSet, CoreSpaceProtocolError> {
    let mut candidates = BTreeMap::<Address, usize>::new();
    for event in execution.trace().events() {
        let crate::execution::TraceEvent::FrameStart { position, frame_id } = event else {
            continue;
        };
        let Some(frame) = execution.trace().try_frame(*frame_id) else {
            return Err(CoreSpaceProtocolError::inconsistent_execution(
                "contract-admin candidate references a missing frame",
            ));
        };
        if frame.space != Space::Native {
            continue;
        }
        let crate::execution::FrameAction::Call {
            call_type,
            target,
            code_address,
            calldata,
            ..
        } = &frame.action
        else {
            continue;
        };
        if *target != ADMIN_CONTROL_CONTRACT_ADDRESS || *code_address != *target {
            continue;
        }
        let contract = decode_contract(calldata)?;
        if let Some(contract) = contract {
            if *call_type != CallType::Call {
                return Err(CoreSpaceProtocolError::inconsistent_execution(
                    "contract-admin call did not use canonical CALL",
                ));
            }
            candidates.entry(contract).or_insert(*position);
        }
    }

    let mut ordered = candidates.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(address, position)| (*position, *address));
    let mut builder = CoreSpaceChangeSetBuilder::new();
    for (index, (contract, _)) in ordered.into_iter().enumerate() {
        let initial = state.initial().contract_admin(contract).map_err(|source| {
            CoreSpaceProtocolError::state_access("read initial Core Space contract admin", source)
        })?;
        let finalized = state
            .finalized()
            .contract_admin(contract)
            .map_err(|source| {
                CoreSpaceProtocolError::state_access(
                    "read finalized Core Space contract admin",
                    source,
                )
            })?;
        if initial == finalized {
            continue;
        }
        let contract_address = state.finalized().core_address(contract).map_err(|source| {
            CoreSpaceProtocolError::state_access("convert contract-admin address", source)
        })?;
        let state = finalized
            .exists
            .then(|| {
                finalized
                    .admin
                    .map(|admin| state.finalized().core_address(admin))
                    .transpose()
            })
            .transpose()
            .map_err(|source| {
                CoreSpaceProtocolError::state_access("convert contract admin address", source)
            })?
            .map(|admin| ContractAdminState { admin });
        builder
            .contract_admin(
                CoreSpaceExecutionPosition::from_index(ADMIN_POSITION_BASE + index),
                contract_address,
                state,
            )
            .map_err(|error| CoreSpaceProtocolError::inconsistent_execution(error.to_string()))?;
    }
    Ok(builder.finish())
}

fn decode_contract(calldata: &[u8]) -> Result<Option<Address>, CoreSpaceProtocolError> {
    if calldata.len() < 4 {
        return Ok(None);
    }
    let selector = &calldata[..4];
    let address = if selector == AdminCalls::setAdminCall::SELECTOR {
        AdminCalls::setAdminCall::abi_decode_validate(calldata)
            .map(|call| call.contract_address)
            .map_err(|error| {
                CoreSpaceProtocolError::inconsistent_execution(format!(
                    "invalid setAdmin calldata: {error}"
                ))
            })?
    } else if selector == AdminCalls::destroyCall::SELECTOR {
        AdminCalls::destroyCall::abi_decode_validate(calldata)
            .map(|call| call.contract_address)
            .map_err(|error| {
                CoreSpaceProtocolError::inconsistent_execution(format!(
                    "invalid destroy calldata: {error}"
                ))
            })?
    } else {
        return Ok(None);
    };
    Ok(Some(Address::from_slice(address.as_slice())))
}

#[cfg(test)]
mod tests {
    use super::{AdminCalls, decode_contract};
    use alloy_sol_types::SolCall;
    use cfx_types::Address;

    #[test]
    fn decodes_admin_targets_and_rejects_noncanonical_payloads() {
        let contract = Address::from_low_u64_be(7);
        let call = AdminCalls::destroyCall {
            contract_address: alloy_primitives::Address::from_slice(&contract.0),
        };
        assert_eq!(decode_contract(&call.abi_encode()).unwrap(), Some(contract));

        let mut malformed = call.abi_encode();
        malformed[4] = 1;
        assert!(decode_contract(&malformed).is_err());
        assert_eq!(decode_contract(&malformed[..3]).unwrap(), None);
    }
}
