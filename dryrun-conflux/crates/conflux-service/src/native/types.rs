use conflux_engine as engine;

pub use engine::{
    AccessListItem, NativeEpochRef, NativeExecution, NativeExecutionFailure,
    NativeExecutionFailureCode, NativeExecutionStatus, NativeStateAnchor, NativeStorageChange,
    NativeTransaction, NativeTransactionVariant, SimulateNativeTransactionInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateNativeTransactionOutput {
    pub execution: NativeExecution,
}

impl From<engine::NativeSimulation> for SimulateNativeTransactionOutput {
    fn from(simulation: engine::NativeSimulation) -> Self {
        Self {
            execution: simulation.into_execution(),
        }
    }
}
