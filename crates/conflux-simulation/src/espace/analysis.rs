use super::{
    EspaceBlockContext, EspaceExecutedTransaction, EspaceLogCheckpoint, EspaceStateAccess,
    EspaceTypedTransaction,
};

#[derive(Clone, Copy)]
pub struct EspaceAnalysisView<'a> {
    pub(crate) context: &'a EspaceBlockContext,
    pub(crate) transaction: &'a EspaceTypedTransaction,
    pub(crate) execution: &'a EspaceExecutedTransaction,
}

impl<'a> EspaceAnalysisView<'a> {
    pub fn context(self) -> &'a EspaceBlockContext {
        self.context
    }
    pub fn transaction(self) -> &'a EspaceTypedTransaction {
        self.transaction
    }
    pub fn execution(self) -> &'a EspaceExecutedTransaction {
        self.execution
    }
    pub fn state(self) -> &'a EspaceStateAccess {
        self.execution.state()
    }
    pub fn log_checkpoints(self) -> impl Iterator<Item = EspaceLogCheckpoint<'a>> {
        self.execution.log_checkpoints()
    }
}
