use crate::{
    ConfluxSimulationBackend,
    core_space::{CoreSpaceExecution, CoreSpaceStateAnchor, PreparedStoragePayer},
    espace::{EspaceBlockContext, EspaceExecution},
    execution::TransactionExecutionInput,
    state::ConfluxStateSource,
};

pub struct PreparedEspaceSimulation {
    pub(crate) state: PreparedEspaceSimulationState,
}

pub(crate) enum PreparedEspaceSimulationState {
    Finished(Box<EspaceExecution>),
    Ready(Box<ReadyEspaceSimulation>),
}

pub(crate) struct ReadyEspaceSimulation {
    pub(crate) backend: ConfluxSimulationBackend,
    pub(crate) chain_id: u32,
    pub(crate) simulated_block: EspaceBlockContext,
    pub(crate) gas_limit: u64,
    pub(crate) execution_input: TransactionExecutionInput,
    pub(crate) state_source: ConfluxStateSource,
}

pub struct PreparedCoreSpaceSimulation {
    pub(crate) state: PreparedCoreSpaceSimulationState,
}

pub(crate) enum PreparedCoreSpaceSimulationState {
    Finished(Box<CoreSpaceExecution>),
    Ready(Box<ReadyCoreSpaceSimulation>),
}

pub(crate) struct ReadyCoreSpaceSimulation {
    pub(crate) backend: ConfluxSimulationBackend,
    pub(crate) chain_id: u32,
    pub(crate) state_anchor: CoreSpaceStateAnchor,
    pub(crate) gas_limit: u64,
    pub(crate) storage_payer: PreparedStoragePayer,
    pub(crate) execution_input: TransactionExecutionInput,
    pub(crate) state_source: ConfluxStateSource,
}
