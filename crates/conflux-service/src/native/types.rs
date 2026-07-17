use conflux_engine as engine;

pub use engine::native::{
    AccessListItem, NativeEpochRef, NativeExecution, NativeExecutionFailure,
    NativeExecutionFailureCode, NativeExecutionStatus, NativeStateAnchor, NativeStorageChange,
    NativeTransaction, NativeTransactionVariant, SimulateNativeTransactionInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateNativeTransactionOutput {
    pub execution: NativeExecution,
}

impl From<engine::native::NativeExecution> for SimulateNativeTransactionOutput {
    fn from(execution: engine::native::NativeExecution) -> Self {
        Self { execution }
    }
}
