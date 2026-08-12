use crate::core_space::changes::ChangePosition;
use cfx_executor::executive_observer::AddressPocket;
use cfx_executor::machine::Machine;
use cfx_types::{AddressSpaceUtil, Space};
use cfx_vm_types::{CallType, Spec};

use super::{
    CommittedPoSCall, CommittedStakingCall,
    codec::{PoSCall, StakingCall, decode_pos_call, decode_staking_call},
};
use crate::{
    core_space::CoreSpaceChangesError,
    execution::{CommittedExecutionTrace, FrameAction, TraceEvent},
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveContracts {
    staking: bool,
    pos_register: bool,
}

impl ActiveContracts {
    pub(crate) fn from_machine_and_spec(machine: &Machine, spec: &Spec) -> Self {
        let internal_contracts = machine.internal_contracts();
        let staking =
            cfx_parameters::internal_contract_addresses::STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS;
        let pos_register =
            cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS;
        Self {
            staking: internal_contracts
                .contract(&staking.with_native_space(), spec)
                .is_some(),
            pos_register: internal_contracts
                .contract(&pos_register.with_native_space(), spec)
                .is_some(),
        }
    }

    pub(crate) const fn staking_is_active(self) -> bool {
        self.staking
    }

    pub(crate) const fn pos_register_is_active(self) -> bool {
        self.pos_register
    }
}

#[derive(Debug, Default)]
pub(crate) struct CommittedCalls {
    staking_calls: Vec<CommittedStakingCall>,
    pos_calls: Vec<CommittedPoSCall>,
}

impl CommittedCalls {
    pub(crate) fn staking_calls(&self) -> &[CommittedStakingCall] {
        &self.staking_calls
    }

    pub(crate) fn pos_calls(&self) -> &[CommittedPoSCall] {
        &self.pos_calls
    }
}

pub(crate) fn collect_calls(
    trace: &CommittedExecutionTrace,
    active_contracts: ActiveContracts,
) -> Result<CommittedCalls, CoreSpaceChangesError> {
    let staking_contract_address =
        cfx_parameters::internal_contract_addresses::STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS;
    let pos_register_contract_address =
        cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS;
    let mut calls = CommittedCalls::default();

    for event in trace.events() {
        let TraceEvent::FrameStart { position, frame_id } = event else {
            continue;
        };
        let frame = trace.frame(*frame_id);
        let FrameAction::Call {
            caller,
            target,
            code_address,
            calldata_len,
            calldata_prefix,
            call_type,
            transferred_value,
        } = &frame.action
        else {
            continue;
        };
        let space = frame.space;

        let is_canonical_plain_call = |expected_address| {
            space == Space::Native
                && *call_type == CallType::Call
                && *target == expected_address
                && *code_address == expected_address
                && transferred_value.is_zero()
        };

        if active_contracts.staking_is_active()
            && (*target == staking_contract_address || *code_address == staking_contract_address)
        {
            if let Some(staking_call) = decode_staking_call(*calldata_len, calldata_prefix)? {
                verify_canonical_plain_call(
                    "staking",
                    is_canonical_plain_call(staking_contract_address),
                )?;
                calls.staking_calls.push(collect_staking_call(
                    trace,
                    *position,
                    *frame_id,
                    *caller,
                    staking_call,
                )?);
            }
            continue;
        }

        if !active_contracts.pos_register_is_active()
            || (*target != pos_register_contract_address
                && *code_address != pos_register_contract_address)
        {
            continue;
        }
        let Some(pos_call) = decode_pos_call(*calldata_len, calldata_prefix)? else {
            continue;
        };
        verify_canonical_plain_call(
            "PoS",
            is_canonical_plain_call(pos_register_contract_address),
        )?;
        let position = ChangePosition::new(*position, 0);
        let account = address_from_cfx(*caller);
        let committed_call = match pos_call {
            PoSCall::Registration {
                pos_identifier,
                vote_count,
            } => CommittedPoSCall::Registration {
                position,
                account,
                pos_identifier,
                vote_count,
            },
            PoSCall::StakeIncrease { vote_count } => CommittedPoSCall::StakeIncrease {
                position,
                account,
                vote_count,
            },
            PoSCall::RetirementRequest {
                requested_vote_count,
            } => CommittedPoSCall::RetirementRequest {
                position,
                account,
                requested_vote_count,
            },
        };
        calls.pos_calls.push(committed_call);
    }

    Ok(calls)
}

fn collect_staking_call(
    trace: &CommittedExecutionTrace,
    position: usize,
    frame_id: crate::execution::FrameId,
    caller: cfx_types::Address,
    call: StakingCall,
) -> Result<CommittedStakingCall, CoreSpaceChangesError> {
    let account = address_from_cfx(caller);
    let frame_transfers: Vec<_> = trace.internal_transfers_in_scope(Some(frame_id)).collect();
    let position = ChangePosition::new(position, 0);

    match call {
        StakingCall::Deposit { amount } => {
            let [transfer] = frame_transfers.as_slice() else {
                return Err(transfer_count_mismatch("deposit", 1, frame_transfers.len()));
            };
            let TraceEvent::InternalTransfer {
                position: transfer_position,
                space: Space::Native,
                from: AddressPocket::Balance(from),
                to: AddressPocket::StakingBalance(to),
                value,
                ..
            } = transfer
            else {
                return Err(invalid_staking_transfer_shape("deposit"));
            };
            if from.space != Space::Native
                || from.address != caller
                || *to != caller
                || u256_from_cfx(*value) != amount
            {
                return Err(invalid_staking_transfer_shape("deposit"));
            }
            Ok(CommittedStakingCall::Deposit {
                position,
                account,
                amount,
                transfer_position: *transfer_position,
            })
        }
        StakingCall::Withdrawal { principal_amount } => {
            let [principal_transfer, reward_transfer] = frame_transfers.as_slice() else {
                return Err(transfer_count_mismatch(
                    "withdrawal",
                    2,
                    frame_transfers.len(),
                ));
            };
            let TraceEvent::InternalTransfer {
                position: principal_transfer_position,
                space: Space::Native,
                from: AddressPocket::StakingBalance(from),
                to: AddressPocket::Balance(to),
                value: principal_value,
                ..
            } = principal_transfer
            else {
                return Err(invalid_staking_transfer_shape("withdrawal principal"));
            };
            let TraceEvent::InternalTransfer {
                position: reward_transfer_position,
                space: Space::Native,
                from: AddressPocket::MintBurn,
                to: AddressPocket::Balance(reward_recipient),
                value: reward_value,
                ..
            } = reward_transfer
            else {
                return Err(invalid_staking_transfer_shape("withdrawal reward"));
            };
            if *from != caller
                || to.space != Space::Native
                || to.address != caller
                || reward_recipient.space != Space::Native
                || reward_recipient.address != caller
                || u256_from_cfx(*principal_value) != principal_amount
            {
                return Err(invalid_staking_transfer_shape("withdrawal"));
            }
            Ok(CommittedStakingCall::Withdrawal {
                position,
                account,
                principal_amount,
                reward_amount: u256_from_cfx(*reward_value),
                principal_transfer_position: *principal_transfer_position,
                reward_transfer_position: *reward_transfer_position,
            })
        }
        StakingCall::VoteLock {
            required_locked_amount,
            unlock_block_number,
        } => {
            if !frame_transfers.is_empty() {
                return Err(transfer_count_mismatch(
                    "voteLock",
                    0,
                    frame_transfers.len(),
                ));
            }
            Ok(CommittedStakingCall::VoteLock {
                position,
                account,
                required_locked_amount,
                unlock_block_number,
            })
        }
    }
}

fn transfer_count_mismatch(
    operation: &str,
    expected_transfer_count: usize,
    actual_transfer_count: usize,
) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space staking {operation} expected {expected_transfer_count} internal transfers in its frame, got {actual_transfer_count}"
    ))
}

fn invalid_staking_transfer_shape(operation: &str) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space staking {operation} did not use the canonical caller, amount, and pocket movement"
    ))
}

fn verify_canonical_plain_call(
    contract_name: &str,
    is_canonical_plain_call: bool,
) -> Result<(), CoreSpaceChangesError> {
    if !is_canonical_plain_call {
        return Err(CoreSpaceChangesError::unsupported_operation(format!(
            "Core Space {contract_name} call did not use the canonical native plain-call form"
        )));
    }
    Ok(())
}
