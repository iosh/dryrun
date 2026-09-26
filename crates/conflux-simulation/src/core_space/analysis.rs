use super::{
    CoreSpaceBlockContext, CoreSpaceExecutedTransaction, CoreSpaceLogCheckpoint,
    CoreSpaceStateAccess, CoreSpaceTypedTransaction,
};

#[derive(Clone, Copy)]
pub struct CoreSpaceAnalysisView<'a> {
    pub(crate) context: &'a CoreSpaceBlockContext,
    pub(crate) transaction: &'a CoreSpaceTypedTransaction,
    pub(crate) execution: &'a CoreSpaceExecutedTransaction,
}

impl<'a> CoreSpaceAnalysisView<'a> {
    pub fn context(self) -> &'a CoreSpaceBlockContext {
        self.context
    }
    pub fn transaction(self) -> &'a CoreSpaceTypedTransaction {
        self.transaction
    }
    pub fn execution(self) -> &'a CoreSpaceExecutedTransaction {
        self.execution
    }
    pub fn state(self) -> &'a CoreSpaceStateAccess {
        self.execution.state()
    }
    pub fn log_checkpoints(self) -> impl Iterator<Item = CoreSpaceLogCheckpoint<'a>> {
        self.execution.log_checkpoints()
    }
}
