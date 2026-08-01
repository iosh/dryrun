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

    pub fn changes(&self) -> &[CoreSpaceChange] {
        &self.changes
    }

    pub fn into_parts(self) -> (CoreSpaceExecution, Vec<CoreSpaceChange>) {
        (self.execution, self.changes)
    }
}
