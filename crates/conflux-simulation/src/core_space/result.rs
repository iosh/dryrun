use super::{
    CoreSpaceBlockContext, CoreSpaceCompleteTransaction, CoreSpaceExecution,
    changes::CoreSpaceChange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulation {
    pub context: CoreSpaceBlockContext,
    pub transaction: CoreSpaceCompleteTransaction,
    execution: CoreSpaceExecution,
    changes: Vec<CoreSpaceChange>,
}

impl CoreSpaceSimulation {
    pub(crate) fn new(
        context: CoreSpaceBlockContext,
        transaction: CoreSpaceCompleteTransaction,
        execution: CoreSpaceExecution,
        changes: Vec<CoreSpaceChange>,
    ) -> Self {
        Self {
            context,
            transaction,
            execution,
            changes,
        }
    }

    pub fn execution(&self) -> &CoreSpaceExecution {
        &self.execution
    }

    pub fn changes(&self) -> &[CoreSpaceChange] {
        &self.changes
    }

    pub fn into_parts(
        self,
    ) -> (
        CoreSpaceBlockContext,
        CoreSpaceCompleteTransaction,
        CoreSpaceExecution,
        Vec<CoreSpaceChange>,
    ) {
        (self.context, self.transaction, self.execution, self.changes)
    }
}
