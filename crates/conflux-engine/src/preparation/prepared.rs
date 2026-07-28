use cfx_types::U256;

use crate::{
    core_space::{CoreSpaceExecution, CoreSpaceStateAnchor},
    espace::{EspaceExecution, SimulatedBlock},
    execution::TransactionExecutionInput,
    state::RemoteStateReader,
};

pub struct PreparedEspaceSimulation {
    pub(crate) state: PreparedEspaceSimulationState,
}

pub(crate) enum PreparedEspaceSimulationState {
    Finished(Box<EspaceExecution>),
    Ready(Box<ReadyEspaceSimulation>),
}

pub(crate) struct ReadyEspaceSimulation {
    pub(crate) chain_id: u32,
    pub(crate) simulated_block: SimulatedBlock,
    pub(crate) gas_limit: U256,
    pub(crate) execution_input: TransactionExecutionInput,
    pub(crate) state_reader: RemoteStateReader,
}

pub struct PreparedCoreSpaceSimulation {
    pub(crate) state: PreparedCoreSpaceSimulationState,
}

pub(crate) enum PreparedCoreSpaceSimulationState {
    Finished(Box<CoreSpaceExecution>),
    Ready(Box<ReadyCoreSpaceSimulation>),
}

pub(crate) struct ReadyCoreSpaceSimulation {
    pub(crate) chain_id: u32,
    pub(crate) state_anchor: CoreSpaceStateAnchor,
    pub(crate) gas_limit: U256,
    pub(crate) execution_input: TransactionExecutionInput,
    pub(crate) state_reader: RemoteStateReader,
}
