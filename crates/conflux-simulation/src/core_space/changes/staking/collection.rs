use cfx_executor::machine::Machine;
use cfx_types::{AddressSpaceUtil, Space};
use cfx_vm_types::{CallType, Spec};

use super::{
    StakingCall,
    codec::{PoSCall, decode_pos_call, decode_vote_lock_call},
};
use crate::{ConfluxSimulationError, execution::Observation, primitive::address_from_cfx};

#[derive(Debug, Clone, Copy)]
pub(crate) struct StakingContractActivation {
    staking: bool,
    pos_register: bool,
}

impl StakingContractActivation {
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
pub(crate) struct CommittedStakingCalls {
    calls: Vec<StakingCall>,
}

impl CommittedStakingCalls {
    pub(super) fn iter(&self) -> impl Iterator<Item = &StakingCall> {
        self.calls.iter()
    }

    pub(crate) fn has_pos_calls(&self) -> bool {
        self.calls.iter().any(|committed_call| {
            matches!(
                committed_call,
                StakingCall::PoSRegistration { .. }
                    | StakingCall::PoSStakeIncrease { .. }
                    | StakingCall::PoSRetirementRequest { .. }
            )
        })
    }
}

pub(crate) fn collect_committed_staking_calls(
    observations: &[Observation],
    contract_activation: StakingContractActivation,
) -> Result<CommittedStakingCalls, ConfluxSimulationError> {
    let staking_contract_address =
        cfx_parameters::internal_contract_addresses::STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS;
    let pos_register_contract_address =
        cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS;
    let mut committed_staking_calls = CommittedStakingCalls::default();

    for observation in observations {
        let Observation::Call {
            position,
            caller,
            target,
            code_address,
            input_len,
            input_prefix,
            space,
            call_type,
            transferred_value,
            ..
        } = observation
        else {
            continue;
        };

        let uses_canonical_plain_call = |expected_address| {
            *space == Space::Native
                && *call_type == CallType::Call
                && *target == expected_address
                && *code_address == expected_address
                && transferred_value.is_zero()
        };

        if contract_activation.staking_is_active()
            && (*target == staking_contract_address || *code_address == staking_contract_address)
        {
            if let Some(vote_lock) = decode_vote_lock_call(*input_len, input_prefix)? {
                validate_committed_call(
                    "voteLock",
                    uses_canonical_plain_call(staking_contract_address),
                )?;
                committed_staking_calls.calls.push(StakingCall::VoteLock {
                    position: contract_standards::Position::new(*position, 0),
                    account: address_from_cfx(*caller),
                    amount: vote_lock.amount,
                    unlock_block_number: vote_lock.unlock_block_number,
                });
            }
            continue;
        }

        if !contract_activation.pos_register_is_active()
            || (*target != pos_register_contract_address
                && *code_address != pos_register_contract_address)
        {
            continue;
        }
        let Some(pos_call) = decode_pos_call(*input_len, input_prefix)? else {
            continue;
        };
        validate_committed_call(
            "PoS",
            uses_canonical_plain_call(pos_register_contract_address),
        )?;
        let position = contract_standards::Position::new(*position, 0);
        let account = address_from_cfx(*caller);
        let committed_call = match pos_call {
            PoSCall::Registration {
                pos_identifier,
                vote_count,
            } => StakingCall::PoSRegistration {
                position,
                account,
                pos_identifier,
                vote_count,
            },
            PoSCall::StakeIncrease { vote_count } => StakingCall::PoSStakeIncrease {
                position,
                account,
                vote_count,
            },
            PoSCall::RetirementRequest {
                requested_vote_count,
            } => StakingCall::PoSRetirementRequest {
                position,
                account,
                requested_vote_count,
            },
        };
        committed_staking_calls.calls.push(committed_call);
    }

    Ok(committed_staking_calls)
}

fn validate_committed_call(
    name: &str,
    uses_canonical_plain_call: bool,
) -> Result<(), ConfluxSimulationError> {
    if !uses_canonical_plain_call {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space {name} call did not use the canonical native plain-call form"
        )));
    }
    Ok(())
}
