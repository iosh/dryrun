use alloy_primitives::U256;
use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use contract_standards::{
    MetadataRequests, StandardCandidate, StandardStateValues, StatePhase, StateRequirements,
    state_requirements, verify,
};

use crate::{
    ConfluxSimulationError,
    execution::{
        ConfluxTransactionExecution, PreparedTransactionExecution, TransactionExecutionOutcome,
    },
    standards::{collect_standard_candidates, load_change_metadata, read_standard_state_values},
    state::{AnchoredVoteLists, MaskedSponsorWhitelistEntries, StatePhaseValues},
};

use super::changes::{
    CfxBalanceLocation, CfxOperations, CfxStateValues, CommittedStakingCalls, CoreSpaceChange,
    PoSEvent, PoSStateRequirements, PoSStateValues, PositionedCoreSpaceChange,
    StakingContractActivation, collect_cfx_operations, collect_committed_staking_calls,
    decode_pos_staking_events, determine_gas_fee_payer, order_and_enrich_core_space_changes,
    read_cfx_state_values, read_pos_state_values, verify_cfx_changes, verify_pos_staking_changes,
    verify_vote_lock_changes,
};

pub(super) struct CoreSpaceExecutionFacts {
    pub(super) expected_gas_fee_payer: CfxBalanceLocation,
    pub(super) cfx_operations: CfxOperations,
    pub(super) committed_staking_calls: CommittedStakingCalls,
    pub(super) pos_staking_events: Vec<PoSEvent>,
    pub(super) pos_state_requirements: PoSStateRequirements,
    pub(super) standard_candidates: Vec<StandardCandidate>,
    pub(super) standard_state_requirements: StateRequirements,
    pub(super) execution_fee: U256,
    pub(super) burnt_fee: Option<U256>,
}

impl CoreSpaceExecutionFacts {
    pub(super) fn from_execution(
        execution: &ConfluxTransactionExecution,
        machine: &Machine,
        masked_sponsor_whitelist_entries: &MaskedSponsorWhitelistEntries,
    ) -> Result<Self, ConfluxSimulationError> {
        let TransactionExecutionOutcome::Success(details) = &execution.outcome else {
            return Err(ConfluxSimulationError::ExecutionInternal {
                message: "successful execution facts are required for Core Space analysis".into(),
            });
        };

        let expected_gas_fee_payer =
            determine_gas_fee_payer(&execution.prepared.transaction, details.gas_sponsor_paid)?;
        let cfx_operations = collect_cfx_operations(
            &details.observations,
            &details.contracts_created,
            &details.storage_released,
            machine,
            &execution.prepared.spec,
        )?;
        cfx_operations
            .reject_masked_sponsorship_access_dependencies(masked_sponsor_whitelist_entries)?;

        let staking_contract_activation =
            StakingContractActivation::from_machine_and_spec(machine, &execution.prepared.spec);
        let committed_staking_calls =
            collect_committed_staking_calls(&details.observations, staking_contract_activation)?;
        let pos_staking_events = decode_pos_staking_events(
            &details.logs,
            staking_contract_activation.pos_register_is_active(),
        )?;
        let pos_state_requirements =
            PoSStateRequirements::from_committed_calls(&committed_staking_calls);
        let standard_candidates =
            collect_standard_candidates(&details.observations, Space::Native)?;
        let standard_state_requirements = state_requirements(&standard_candidates);

        Ok(Self {
            expected_gas_fee_payer,
            cfx_operations,
            committed_staking_calls,
            pos_staking_events,
            pos_state_requirements,
            standard_candidates,
            standard_state_requirements,
            execution_fee: details.common.fee,
            burnt_fee: details.common.burnt_fee,
        })
    }
}

#[derive(Debug)]
pub(super) struct CoreSpaceStateValues {
    pub(super) cfx: CfxStateValues,
    pub(super) pos: Option<PoSStateValues>,
    pub(super) standards: StandardStateValues,
}

#[derive(Default)]
pub(super) struct CoreSpaceStateReader {
    before_pos_state: Option<PoSStateValues>,
}

impl CoreSpaceStateReader {
    pub(super) fn read(
        &mut self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        facts: &CoreSpaceExecutionFacts,
        phase: StatePhase,
    ) -> Result<CoreSpaceStateValues, ConfluxSimulationError> {
        let cfx = read_cfx_state_values(state, phase, &facts.cfx_operations)?;
        let pos = self.read_pos_state(state, facts, phase)?;
        let standards = read_standard_state_values(
            state,
            machine,
            prepared_execution,
            phase,
            &facts.standard_state_requirements,
        )?;

        Ok(CoreSpaceStateValues {
            cfx,
            pos,
            standards,
        })
    }

    fn read_pos_state(
        &mut self,
        state: &State,
        facts: &CoreSpaceExecutionFacts,
        phase: StatePhase,
    ) -> Result<Option<PoSStateValues>, ConfluxSimulationError> {
        if !facts.committed_staking_calls.has_pos_calls() {
            self.before_pos_state = None;
            return Ok(None);
        }

        match phase {
            StatePhase::Before => {
                let before = read_pos_state_values(
                    state,
                    StatePhase::Before,
                    &facts.pos_state_requirements,
                )?;
                self.before_pos_state = Some(before.clone());
                Ok(Some(before))
            }
            StatePhase::After => {
                let Some(before) = self.before_pos_state.as_ref() else {
                    return Err(ConfluxSimulationError::ExecutionInternal {
                        message: "Core Space PoS before state was not collected".into(),
                    });
                };
                let requirements = facts
                    .pos_state_requirements
                    .including_identifiers_from(before);
                Ok(Some(read_pos_state_values(
                    state,
                    StatePhase::After,
                    &requirements,
                )?))
            }
        }
    }
}

pub(super) fn analyze_core_space_changes(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    facts: CoreSpaceExecutionFacts,
    phase_values: StatePhaseValues<CoreSpaceStateValues>,
    anchored_vote_lists: &AnchoredVoteLists,
) -> Result<Vec<CoreSpaceChange>, ConfluxSimulationError> {
    let StatePhaseValues { before, after } = phase_values;
    let CoreSpaceStateValues {
        cfx: before_cfx_state,
        pos: before_pos_state,
        standards: before_standard_state,
    } = before;
    let CoreSpaceStateValues {
        cfx: after_cfx_state,
        pos: after_pos_state,
        standards: after_standard_state,
    } = after;

    let positioned_standard_changes = verify(
        &facts.standard_candidates,
        &before_standard_state,
        &after_standard_state,
    )?;
    let metadata_requests = MetadataRequests::from_changes(&positioned_standard_changes);
    let mut positioned_core_changes = verify_cfx_changes(
        &facts.cfx_operations,
        &before_cfx_state,
        &after_cfx_state,
        facts.expected_gas_fee_payer,
        facts.execution_fee,
        facts.burnt_fee,
    )?;
    positioned_core_changes.extend(verify_vote_lock_changes(
        state,
        &facts.committed_staking_calls,
        anchored_vote_lists,
        prepared_execution.env.number,
    )?);

    match (before_pos_state, after_pos_state) {
        (Some(before), Some(after)) => {
            positioned_core_changes.extend(verify_pos_staking_changes(
                &facts.committed_staking_calls,
                &facts.pos_staking_events,
                &before,
                &after,
                &facts.cfx_operations,
            )?);
        }
        (None, None) if facts.pos_staking_events.is_empty() => {}
        (None, None) => {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space PoS final logs had no matching committed call",
            ));
        }
        _ => {
            return Err(ConfluxSimulationError::ExecutionInternal {
                message: "Core Space PoS before and after states were inconsistent".into(),
            });
        }
    }

    positioned_core_changes.extend(
        positioned_standard_changes
            .into_iter()
            .map(PositionedCoreSpaceChange::from),
    );

    if positioned_core_changes.is_empty() {
        return Ok(Vec::new());
    }

    let metadata = load_change_metadata(state, machine, prepared_execution, &metadata_requests)?;
    Ok(order_and_enrich_core_space_changes(
        positioned_core_changes,
        &metadata,
    ))
}
