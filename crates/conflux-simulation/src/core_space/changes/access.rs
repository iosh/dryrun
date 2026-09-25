use std::collections::BTreeMap;

use alloy_sol_types::{SolCall, sol};
use cfx_parameters::internal_contract_addresses::{
    ADMIN_CONTROL_CONTRACT_ADDRESS, SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS,
};
use cfx_types::{Address, Space};
use cfx_vm_types::CallType;

use crate::{
    core_space::{
        CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceProtocolError,
        CoreSpaceStateAccess, SponsorshipAccessRuleScope,
    },
    execution::{FrameAction, TraceEvent},
    state::SponsorWhitelistStorageKey,
};

use super::{ACCESS_RULE_POSITION_BASE, CoreSpaceChangeSet, CoreSpaceChangeSetBuilder};

sol! {
    interface AccessRuleCalls {
        function addPrivilege(address[] account_addresses) external;
        function removePrivilege(address[] account_addresses) external;
        function addPrivilegeByAdmin(address contract_address, address[] account_addresses) external;
        function removePrivilegeByAdmin(address contract_address, address[] account_addresses) external;
    }

    interface AdminCalls {
        function destroy(address contract_address) external;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CandidateKey {
    contract_address: Address,
    account_address: Address,
}

pub(super) fn derive_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
) -> Result<CoreSpaceChangeSet, CoreSpaceProtocolError> {
    let mut candidates = BTreeMap::<CandidateKey, usize>::new();
    for event in execution.trace().events() {
        match event {
            TraceEvent::StorageWrite {
                address,
                key,
                position,
                ..
            } if address.space == Space::Native
                && address.address == SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS =>
            {
                if key.len() == 40 {
                    let candidate = CandidateKey {
                        contract_address: Address::from_slice(&key[..20]),
                        account_address: Address::from_slice(&key[20..]),
                    };
                    candidates.entry(candidate).or_insert(*position);
                }
            }
            TraceEvent::FrameStart { position, frame_id } => {
                let Some(frame) = execution.trace().try_frame(*frame_id) else {
                    return Err(CoreSpaceProtocolError::inconsistent_execution(
                        "sponsorship access candidate references a missing frame",
                    ));
                };
                if frame.space != Space::Native {
                    continue;
                }
                let FrameAction::Call {
                    call_type,
                    caller,
                    target,
                    code_address,
                    calldata,
                    ..
                } = &frame.action
                else {
                    continue;
                };
                if *target == ADMIN_CONTROL_CONTRACT_ADDRESS
                    && *code_address == ADMIN_CONTROL_CONTRACT_ADDRESS
                    && is_destroy(calldata)?
                {
                    if *call_type != CallType::Call {
                        return Err(CoreSpaceProtocolError::inconsistent_execution(
                            "contract-destroy call did not use canonical CALL",
                        ));
                    }
                    return Err(CoreSpaceProtocolError::unsupported_operation(
                        "contract destruction may clear an unenumerable sponsorship whitelist range",
                    ));
                }
                if *target != SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS
                    || *code_address != SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS
                {
                    continue;
                }
                let decoded = decode_access_call(calldata, *caller)?;
                if decoded.is_empty() {
                    continue;
                }
                if *call_type != CallType::Call {
                    return Err(CoreSpaceProtocolError::inconsistent_execution(
                        "sponsorship access call did not use canonical CALL",
                    ));
                }
                for (contract_address, account_addresses) in decoded {
                    for account_address in account_addresses {
                        candidates
                            .entry(CandidateKey {
                                contract_address,
                                account_address,
                            })
                            .or_insert(*position);
                    }
                }
            }
            _ => {}
        }
    }

    let mut builder = CoreSpaceChangeSetBuilder::new();
    let mut ordered = candidates.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(candidate, position)| (*position, *candidate));
    for (index, (candidate, _)) in ordered.into_iter().enumerate() {
        let key = SponsorWhitelistStorageKey {
            contract_address: candidate.contract_address,
            account_address: candidate.account_address,
        };
        let initial = state
            .initial()
            .sponsorship_access_rule(key)
            .map_err(|source| {
                CoreSpaceProtocolError::state_access("read initial sponsorship access rule", source)
            })?;
        let finalized = state
            .finalized()
            .sponsorship_access_rule(key)
            .map_err(|source| {
                CoreSpaceProtocolError::state_access(
                    "read finalized sponsorship access rule",
                    source,
                )
            })?;
        if !candidate.account_address.is_zero()
            && state
                .masked_whitelist_keys()
                .map_err(|source| {
                    CoreSpaceProtocolError::state_access(
                        "snapshot masked sponsorship access rules",
                        source,
                    )
                })?
                .contains(&key)
        {
            return Err(CoreSpaceProtocolError::inconsistent_execution(
                "sponsorship access rule depends on a user whitelist entry masked by the all-accounts rule",
            ));
        }
        if initial == finalized {
            continue;
        }
        let contract_address = state
            .finalized()
            .core_address(candidate.contract_address)
            .map_err(|source| {
                CoreSpaceProtocolError::state_access("convert sponsorship contract address", source)
            })?;
        let scope = if candidate.account_address.is_zero() {
            SponsorshipAccessRuleScope::AllAccounts
        } else {
            SponsorshipAccessRuleScope::Account(
                state
                    .finalized()
                    .core_address(candidate.account_address)
                    .map_err(|source| {
                        CoreSpaceProtocolError::state_access(
                            "convert sponsorship account address",
                            source,
                        )
                    })?,
            )
        };
        builder
            .sponsorship_access_rule(
                CoreSpaceExecutionPosition::from_index(ACCESS_RULE_POSITION_BASE + index),
                contract_address,
                scope,
                finalized,
            )
            .map_err(|error| CoreSpaceProtocolError::inconsistent_execution(error.to_string()))?;
    }
    Ok(builder.finish())
}

fn decode_access_call(
    calldata: &[u8],
    caller: Address,
) -> Result<Vec<(Address, Vec<Address>)>, CoreSpaceProtocolError> {
    if calldata.len() < 4 {
        return Ok(Vec::new());
    }
    let selector = &calldata[..4];
    let (contract_address, account_addresses) =
        if selector == AccessRuleCalls::addPrivilegeCall::SELECTOR {
            let call = AccessRuleCalls::addPrivilegeCall::abi_decode_validate(calldata).map_err(
                |error| {
                    CoreSpaceProtocolError::inconsistent_execution(format!(
                        "invalid addPrivilege calldata: {error}"
                    ))
                },
            )?;
            (caller, call.account_addresses)
        } else if selector == AccessRuleCalls::removePrivilegeCall::SELECTOR {
            let call = AccessRuleCalls::removePrivilegeCall::abi_decode_validate(calldata)
                .map_err(|error| {
                    CoreSpaceProtocolError::inconsistent_execution(format!(
                        "invalid removePrivilege calldata: {error}"
                    ))
                })?;
            (caller, call.account_addresses)
        } else if selector == AccessRuleCalls::addPrivilegeByAdminCall::SELECTOR {
            let call = AccessRuleCalls::addPrivilegeByAdminCall::abi_decode_validate(calldata)
                .map_err(|error| {
                    CoreSpaceProtocolError::inconsistent_execution(format!(
                        "invalid addPrivilegeByAdmin calldata: {error}"
                    ))
                })?;
            (
                Address::from_slice(call.contract_address.as_slice()),
                call.account_addresses,
            )
        } else if selector == AccessRuleCalls::removePrivilegeByAdminCall::SELECTOR {
            let call = AccessRuleCalls::removePrivilegeByAdminCall::abi_decode_validate(calldata)
                .map_err(|error| {
                CoreSpaceProtocolError::inconsistent_execution(format!(
                    "invalid removePrivilegeByAdmin calldata: {error}"
                ))
            })?;
            (
                Address::from_slice(call.contract_address.as_slice()),
                call.account_addresses,
            )
        } else {
            return Ok(Vec::new());
        };
    Ok(vec![(
        contract_address,
        account_addresses
            .into_iter()
            .map(|address| Address::from_slice(address.as_slice()))
            .collect(),
    )])
}

fn is_destroy(calldata: &[u8]) -> Result<bool, CoreSpaceProtocolError> {
    if calldata.len() < 4 || calldata[..4] != AdminCalls::destroyCall::SELECTOR {
        return Ok(false);
    }
    AdminCalls::destroyCall::abi_decode_validate(calldata)
        .map(|_| true)
        .map_err(|error| {
            CoreSpaceProtocolError::inconsistent_execution(format!(
                "invalid destroy calldata: {error}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::{AccessRuleCalls, decode_access_call};
    use alloy_sol_types::SolCall;
    use cfx_types::Address;

    #[test]
    fn decodes_access_calls_and_rejects_noncanonical_payloads() {
        let caller = Address::from_low_u64_be(1);
        let account = Address::from_low_u64_be(2);
        let call = AccessRuleCalls::addPrivilegeCall {
            account_addresses: vec![alloy_primitives::Address::from_slice(&account.0)],
        };
        let decoded = decode_access_call(&call.abi_encode(), caller).unwrap();
        assert_eq!(decoded, vec![(caller, vec![account])]);

        let mut malformed = call.abi_encode();
        malformed[4] = 1;
        assert!(decode_access_call(&malformed, caller).is_err());
        assert!(
            decode_access_call(&malformed[..3], caller)
                .unwrap()
                .is_empty()
        );
    }
}
