use cfx_executor::executive::ExecutionError;
use cfx_vm_types as vm;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationError,
    execution::{ConfluxExecutionOutcome, ConfluxTransactionExecution},
    preparation::{
        PreparedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
};

use super::{
    CoreSpaceChange, CoreSpaceSimulation, PreparedStoragePayer, build_core_space_execution,
    session::CoreSpaceExecutionSession,
};

pub(crate) fn simulate(
    prepared_simulation: PreparedCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxSimulationError> {
    match prepared_simulation.state {
        PreparedCoreSpaceSimulationState::Finished(finished) => {
            let finished = *finished;
            Ok(CoreSpaceSimulation::new(
                finished.context,
                finished.transaction,
                finished.execution,
                Vec::new(),
            ))
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
        backend,
        chain_id,
        public_context,
        transaction,
        storage_payer,
        execution_block_context,
        state_source,
    } = ready_simulation;
    let session = CoreSpaceExecutionSession::new(&backend, state_source, runtime_handle.clone())?;
    let session_result = session.execute(&transaction, execution_block_context)?;

    Ok(build_core_space_simulation(
        chain_id,
        public_context,
        transaction,
        storage_payer,
        session_result.execution,
        session_result.changes,
    ))
}

fn build_core_space_simulation(
    chain_id: u32,
    public_context: super::CoreSpaceBlockContext,
    transaction: super::CoreSpaceCompleteTransaction,
    storage_payer: PreparedStoragePayer,
    execution: ConfluxTransactionExecution,
    changes: Vec<CoreSpaceChange>,
) -> CoreSpaceSimulation {
    let storage_payer =
        storage_payer_for_outcome(storage_payer, &execution.outcome, &execution.prepared.spec);
    let gas_limit = transaction.gas_limit;
    let core_execution = build_core_space_execution(
        chain_id,
        public_context,
        gas_limit,
        execution.outcome,
        storage_payer,
    );
    CoreSpaceSimulation::new(public_context, transaction, core_execution, changes)
}

fn storage_payer_for_outcome(
    storage_payer: PreparedStoragePayer,
    outcome: &ConfluxExecutionOutcome,
    spec: &cfx_vm_types::Spec,
) -> Option<PreparedStoragePayer> {
    let outcome = match outcome {
        ConfluxExecutionOutcome::Success(_) => StoragePayerOutcome::Success,
        ConfluxExecutionOutcome::Failed {
            error: ExecutionError::VmError(vm::Error::Reverted),
            ..
        } => StoragePayerOutcome::Reverted,
        ConfluxExecutionOutcome::Failed { .. } => StoragePayerOutcome::FullyChargedError,
        ConfluxExecutionOutcome::NotExecutedDrop(_)
        | ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(_) => {
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
