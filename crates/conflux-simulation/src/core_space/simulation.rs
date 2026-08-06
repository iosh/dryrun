use cfx_executor::executive::ExecutionError;
use cfx_types::Space;
use cfx_vm_types as vm;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationError,
    execution::{
        ConfluxTransactionExecution, ConfluxTransactionExecutor, ObservationObserver,
        TransactionExecutionOutcome, build_conflux_state, build_mainnet_machine,
    },
    preparation::{
        PreparedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
    state::execute_with_state_phases,
};

use super::{
    CoreSpaceChange, CoreSpaceSimulation, PreparedStoragePayer,
    analysis::{CoreSpaceAnalysisInput, CoreSpaceStateReader, analyze_core_space_changes},
    build_core_space_execution,
};

pub(crate) fn simulate(
    prepared_simulation: PreparedCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxSimulationError> {
    match prepared_simulation.state {
        PreparedCoreSpaceSimulationState::Finished(core_execution) => {
            Ok(CoreSpaceSimulation::new(*core_execution, Vec::new()))
        }
        PreparedCoreSpaceSimulationState::Ready(ready_simulation) => {
            simulate_ready(*ready_simulation, runtime_handle)
        }
    }
}

fn simulate_ready(
    ready_simulation: ReadyCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxSimulationError> {
    let ReadyCoreSpaceSimulation {
        chain_id,
        state_anchor,
        gas_limit,
        storage_payer,
        execution_input,
        state_source,
    } = ready_simulation;
    let masked_sponsor_whitelist_entries = state_source.masked_sponsor_whitelist_entries();
    let anchored_vote_lists = state_source.anchored_vote_lists();
    let mut state = build_conflux_state(state_source, runtime_handle.clone()).map_err(|error| {
        ConfluxSimulationError::StateAccess {
            message: error.to_string(),
        }
    })?;
    let machine = build_mainnet_machine();
    let mut state_reader = CoreSpaceStateReader::default();
    let (execution, phase_values) = execute_with_state_phases(
        &mut state,
        |state| {
            ConfluxTransactionExecutor::new(state, &machine)
                .execute(execution_input, ObservationObserver::new(Space::Native))
                .map_err(ConfluxSimulationError::from)
        },
        |execution| {
            if !matches!(&execution.outcome, TransactionExecutionOutcome::Success(_)) {
                return Ok(None);
            }
            CoreSpaceAnalysisInput::from_execution(
                execution,
                &machine,
                &masked_sponsor_whitelist_entries,
            )
            .map(Some)
        },
        |state, execution, analysis_input, state_phase| {
            state_reader.read(
                state,
                &machine,
                &execution.prepared,
                analysis_input,
                state_phase,
            )
        },
    )?;

    let Some((analysis_input, phase_values)) = phase_values else {
        return Ok(build_core_space_simulation(
            chain_id,
            state_anchor,
            gas_limit,
            storage_payer,
            execution,
            Vec::new(),
        ));
    };
    let core_changes = analyze_core_space_changes(
        &mut state,
        &machine,
        &execution.prepared,
        analysis_input,
        phase_values,
        &anchored_vote_lists,
    )?;

    Ok(build_core_space_simulation(
        chain_id,
        state_anchor,
        gas_limit,
        storage_payer,
        execution,
        core_changes,
    ))
}

fn build_core_space_simulation(
    chain_id: u32,
    state_anchor: super::CoreSpaceStateAnchor,
    gas_limit: u64,
    storage_payer: PreparedStoragePayer,
    execution: ConfluxTransactionExecution,
    changes: Vec<CoreSpaceChange>,
) -> CoreSpaceSimulation {
    let storage_payer =
        storage_payer_for_outcome(storage_payer, &execution.outcome, &execution.prepared.spec);
    let core_execution = build_core_space_execution(
        chain_id,
        state_anchor,
        gas_limit,
        execution.outcome,
        storage_payer,
    );
    CoreSpaceSimulation::new(core_execution, changes)
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
