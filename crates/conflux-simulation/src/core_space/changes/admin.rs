use std::collections::BTreeMap;

use alloy_sol_types::{SolCall, sol};
use cfx_parameters::internal_contract_addresses::ADMIN_CONTROL_CONTRACT_ADDRESS;
use cfx_types::{Address, Space};
use cfx_vm_types::CallType;

use super::{ContractAdminState, CoreSpaceChangeSet, CoreSpaceChangeSetBuilder};
use crate::core_space::{
    CoreSpaceExecutedTransaction, CoreSpaceProtocolError, CoreSpaceStateAccess,
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
    for (contract, _) in ordered {
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
        builder.contract_admin(
            simulation_core::changes::ChangePosition::Settlement,
            contract_address,
            state,
        );
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
