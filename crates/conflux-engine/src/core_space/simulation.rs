use std::collections::HashSet;

use cfx_executor::{executive::ExecutionError, executive_observer::AddressPocket};
use cfx_types::{Address, Space};
use cfx_vm_types as vm;
use contract_standards::{StatePhase, state_requirements, verify};
use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError,
    execution::{
        ExecutedTransactionDetails, Observation, TransactionExecutionOutcome,
        build_mainnet_machine, build_rpc_backed_state, execute_transaction,
        prepare_transaction_execution,
    },
    preparation::{
        PreparedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
    standards::{collect_standard_candidates, read_standard_state_values},
    state::StoragePointInitializationUncertainty,
};

use super::{
    CoreSpaceSimulation, PreparedStoragePayer, build_core_space_execution,
    changes::{
        PoSStateRequirements, PositionedCoreSpaceChange, StakingContractActivation,
        collect_cfx_operations, collect_committed_staking_calls, decode_pos_staking_events,
        determine_gas_fee_payer, into_ordered_core_space_changes, read_cfx_state_values,
        read_pos_state_values, verify_cfx_changes, verify_pos_staking_changes,
        verify_vote_lock_changes,
    },
};

pub(crate) fn simulate(
    prepared_simulation: PreparedCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxEngineError> {
    match prepared_simulation.state {
        PreparedCoreSpaceSimulationState::Finished(core_execution) => {
            Ok(CoreSpaceSimulation::new(*core_execution, Vec::new()))
        }
        PreparedCoreSpaceSimulationState::Ready(ready_simulation) => {
            let ReadyCoreSpaceSimulation {
                chain_id,
                state_anchor,
                gas_limit,
                storage_payer,
                execution_input,
                state_reader,
            } = *ready_simulation;
            let storage_point_initialization_uncertainty =
                state_reader.storage_point_initialization_uncertainty();
            let anchored_vote_lists = state_reader.anchored_vote_lists();
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle.clone()).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;
            let machine = build_mainnet_machine();
            let prepared_execution =
                prepare_transaction_execution(&state, &machine, execution_input)?;
            let before_execution_snapshot = state.save();
            let mut transaction_outcome =
                execute_transaction(&mut state, &machine, &prepared_execution)
                    .map_err(ConfluxEngineError::from)?;
            validate_storage_point_state_dependencies(
                &transaction_outcome,
                &storage_point_initialization_uncertainty,
            )?;

            let (execution_observations, final_logs, execution_fee, burnt_fee, gas_paid_by_sponsor) =
                match &mut transaction_outcome {
                    TransactionExecutionOutcome::Success(executed_details) => (
                        std::mem::take(&mut executed_details.observations),
                        std::mem::take(&mut executed_details.logs),
                        executed_details.common.fee,
                        executed_details.common.burnt_fee,
                        executed_details.gas_sponsor_paid,
                    ),
                    _ => {
                        let storage_payer = storage_payer_for_outcome(
                            storage_payer,
                            &transaction_outcome,
                            &prepared_execution.spec,
                        );
                        let core_execution = build_core_space_execution(
                            chain_id,
                            state_anchor,
                            gas_limit,
                            transaction_outcome,
                            storage_payer,
                        );
                        return Ok(CoreSpaceSimulation::new(core_execution, Vec::new()));
                    }
                };
            let expected_gas_fee_payer =
                determine_gas_fee_payer(&prepared_execution.transaction, gas_paid_by_sponsor)?;
            let cfx_operations = collect_cfx_operations(
                &execution_observations,
                &machine,
                &prepared_execution.spec,
            )?;
            let staking_contract_activation = StakingContractActivation::from_machine_and_spec(
                &machine,
                &prepared_execution.spec,
            );
            let committed_staking_calls = collect_committed_staking_calls(
                &execution_observations,
                staking_contract_activation,
            )?;
            let pos_staking_events = decode_pos_staking_events(
                &final_logs,
                staking_contract_activation.pos_register_is_active(),
            )?;
            let pos_state_requirements =
                PoSStateRequirements::from_committed_calls(&committed_staking_calls);
            let standard_candidates =
                collect_standard_candidates(execution_observations, Space::Native)?;

            let standard_state_requirements = state_requirements(&standard_candidates);
            let after_execution_snapshot = state.save();

            state.restore(before_execution_snapshot);
            let before_cfx_state =
                read_cfx_state_values(&state, StatePhase::Before, &cfx_operations)?;
            let before_pos_state = committed_staking_calls
                .has_pos_calls()
                .then(|| read_pos_state_values(&state, StatePhase::Before, &pos_state_requirements))
                .transpose()?;
            let before_standard_state = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::Before,
                &standard_state_requirements,
            )?;

            state.restore(after_execution_snapshot);
            let after_cfx_state =
                read_cfx_state_values(&state, StatePhase::After, &cfx_operations)?;
            let pos_states = match before_pos_state {
                Some(before_pos_state) => {
                    let after_pos_state = read_pos_state_values(
                        &state,
                        StatePhase::After,
                        &pos_state_requirements.including_identifiers_from(&before_pos_state),
                    )?;
                    Some((before_pos_state, after_pos_state))
                }
                None => None,
            };
            let after_standard_state = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::After,
                &standard_state_requirements,
            )?;

            let positioned_standard_changes = verify(
                &standard_candidates,
                &before_standard_state,
                &after_standard_state,
            )?;
            let mut positioned_core_changes = verify_cfx_changes(
                &cfx_operations,
                &before_cfx_state,
                &after_cfx_state,
                expected_gas_fee_payer,
                execution_fee,
                burnt_fee,
            )?;
            positioned_core_changes.extend(verify_vote_lock_changes(
                &state,
                &committed_staking_calls,
                &anchored_vote_lists,
                prepared_execution.env.number,
            )?);
            match pos_states {
                Some((before_pos_state, after_pos_state)) => {
                    positioned_core_changes.extend(verify_pos_staking_changes(
                        &committed_staking_calls,
                        &pos_staking_events,
                        &before_pos_state,
                        &after_pos_state,
                        &cfx_operations,
                    )?);
                }
                None if pos_staking_events.is_empty() => {}
                None => {
                    return Err(ConfluxEngineError::analysis_failed(
                        "Core Space PoS final logs had no matching committed call",
                    ));
                }
            }
            positioned_core_changes.extend(
                positioned_standard_changes
                    .into_iter()
                    .map(PositionedCoreSpaceChange::from),
            );
            let ordered_core_changes = into_ordered_core_space_changes(positioned_core_changes);

            let storage_payer = storage_payer_for_outcome(
                storage_payer,
                &transaction_outcome,
                &prepared_execution.spec,
            );
            let core_execution = build_core_space_execution(
                chain_id,
                state_anchor,
                gas_limit,
                transaction_outcome,
                storage_payer,
            );

            Ok(CoreSpaceSimulation::new(
                core_execution,
                ordered_core_changes,
            ))
        }
    }
}

fn storage_payer_for_outcome(
    storage_payer: PreparedStoragePayer,
    outcome: &TransactionExecutionOutcome,
    spec: &cfx_vm_types::Spec,
) -> Option<PreparedStoragePayer> {
    let outcome = match outcome {
        TransactionExecutionOutcome::Success(_) => StoragePayerOutcome::Success,
        TransactionExecutionOutcome::Failed {
            error: ExecutionError::VmError(vm::Error::Reverted),
            ..
        } => StoragePayerOutcome::Reverted,
        TransactionExecutionOutcome::Failed { .. } => StoragePayerOutcome::FullyChargedError,
        TransactionExecutionOutcome::NotExecutedDrop(_)
        | TransactionExecutionOutcome::NotExecutedToReconsiderPacking(_) => {
            StoragePayerOutcome::NotExecuted
        }
    };

    if storage_payer_is_reported(outcome, spec.cip78a, spec.cip78b) {
        Some(storage_payer)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum StoragePayerOutcome {
    Success,
    Reverted,
    FullyChargedError,
    NotExecuted,
}

const fn storage_payer_is_reported(
    outcome: StoragePayerOutcome,
    cip78a: bool,
    cip78b: bool,
) -> bool {
    match outcome {
        StoragePayerOutcome::Success => cip78a,
        StoragePayerOutcome::Reverted => cip78a,
        StoragePayerOutcome::FullyChargedError => cip78b,
        StoragePayerOutcome::NotExecuted => false,
    }
}

fn validate_storage_point_state_dependencies(
    transaction_outcome: &TransactionExecutionOutcome,
    initialization_uncertainty: &StoragePointInitializationUncertainty,
) -> Result<(), ConfluxEngineError> {
    let TransactionExecutionOutcome::Success(executed_details) = transaction_outcome else {
        return Ok(());
    };
    let uncertain_account_addresses = initialization_uncertainty
        .uncertain_account_addresses()
        .map_err(|error| ConfluxEngineError::StateAccess {
            message: error.to_string(),
        })?;

    let Some(account_address) =
        storage_point_dependent_account(executed_details, &uncertain_account_addresses)
    else {
        return Ok(());
    };

    Err(ConfluxEngineError::analysis_failed(format!(
        "Core Space execution depends on storage-point initialization state unavailable from public RPC for account {account_address:?}"
    )))
}

fn storage_point_dependent_account(
    executed_details: &ExecutedTransactionDetails,
    uncertain_account_addresses: &HashSet<Address>,
) -> Option<Address> {
    executed_details
        .storage_collateralized
        .iter()
        .chain(&executed_details.storage_released)
        .find_map(|change| {
            uncertain_account_addresses
                .contains(&change.address)
                .then_some(change.address)
        })
        .or_else(|| {
            executed_details
                .observations
                .iter()
                .find_map(|observation| match observation {
                    Observation::InternalTransfer { from, to, .. } => {
                        storage_point_pocket_account(from)
                            .filter(|address| uncertain_account_addresses.contains(address))
                            .or_else(|| {
                                storage_point_pocket_account(to)
                                    .filter(|address| uncertain_account_addresses.contains(address))
                            })
                    }
                    _ => None,
                })
        })
}

fn storage_point_pocket_account(pocket: &AddressPocket) -> Option<Address> {
    match pocket {
        AddressPocket::StorageCollateral(address)
        | AddressPocket::SponsorBalanceForStorage(address) => Some(*address),
        _ => None,
    }
}
