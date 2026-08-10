use crate::{
    ConfluxSimulationBackend,
    core_space::{CoreSpaceBlockContext, CoreSpaceExecution, PreparedStoragePayer},
    execution::TransactionExecutionInput,
    state::ConfluxStateSource,
};

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
    pub(crate) public_context: CoreSpaceBlockContext,
    pub(crate) gas_limit: u64,
    pub(crate) storage_payer: PreparedStoragePayer,
    pub(crate) execution_input: TransactionExecutionInput,
    pub(crate) state_source: ConfluxStateSource,
}
