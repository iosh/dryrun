use cfx_types::U256;

use crate::{
    core_space::{CoreSpaceExecution, CoreSpaceStateAnchor},
    espace::{EspaceExecution, SimulatedBlock},
    execution::TransactionExecutionInput,
    state::RemoteStateReader,
};

pub struct PreparedEspaceSimulation {
    pub(crate) kind: PreparedEspaceSimulationState,
}

pub(crate) enum PreparedEspaceSimulationState {
    Complete(Box<EspaceExecution>),
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
    pub(crate) kind: PreparedCoreSpaceSimulationState,
}

pub(crate) enum PreparedCoreSpaceSimulationState {
    Complete(Box<CoreSpaceExecution>),
    Ready(Box<ReadyCoreSpaceSimulation>),
}

pub(crate) struct ReadyCoreSpaceSimulation {
    pub(crate) chain_id: u32,
    pub(crate) state_anchor: CoreSpaceStateAnchor,
    pub(crate) gas_limit: U256,
    pub(crate) execution_input: TransactionExecutionInput,
    pub(crate) state_reader: RemoteStateReader,
}
