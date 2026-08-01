use super::{CoreSpaceExecution, changes::CoreSpaceChange};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulation {
    execution: CoreSpaceExecution,
    changes: Vec<CoreSpaceChange>,
}

impl CoreSpaceSimulation {
    pub(crate) fn new(execution: CoreSpaceExecution, changes: Vec<CoreSpaceChange>) -> Self {
        Self { execution, changes }
    }

    pub fn execution(&self) -> &CoreSpaceExecution {
        &self.execution
    }

    pub fn into_execution_only(self) -> CoreSpaceExecution {
        let Self {
            execution,
            changes: unpublished_changes,
        } = self;
        drop(unpublished_changes);
        execution
    }
}
