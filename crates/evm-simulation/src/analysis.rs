use super::{
    EvmBlockContext, EvmLogCheckpoint, EvmStateAccess, EvmTransactionExecution, TypedTransaction,
};

#[derive(Clone, Copy)]
pub struct EvmAnalysisView<'a> {
    pub(crate) context: &'a EvmBlockContext,
    pub(crate) transaction: &'a TypedTransaction,
    pub(crate) execution: &'a EvmTransactionExecution,
}

impl<'a> EvmAnalysisView<'a> {
    pub fn context(self) -> &'a EvmBlockContext {
        self.context
    }
    pub fn transaction(self) -> &'a TypedTransaction {
        self.transaction
    }
    pub fn execution(self) -> &'a EvmTransactionExecution {
        self.execution
    }
    pub fn state(self) -> &'a EvmStateAccess {
        self.execution.state()
    }
    pub fn log_checkpoints(self) -> impl Iterator<Item = EvmLogCheckpoint<'a>> {
        self.execution.log_checkpoints()
    }
}
