use contract_standards::StandardChange;

use super::CoreSpaceExecution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulation {
    pub execution: CoreSpaceExecution,
    pub standard_changes: Vec<StandardChange>,
}

impl CoreSpaceSimulation {
    pub fn new(execution: CoreSpaceExecution, standard_changes: Vec<StandardChange>) -> Self {
        Self {
            execution,
            standard_changes,
        }
    }

    pub fn execution(&self) -> &CoreSpaceExecution {
        &self.execution
    }

    pub fn standard_changes(&self) -> &[StandardChange] {
        &self.standard_changes
    }

    pub fn into_parts(self) -> (CoreSpaceExecution, Vec<StandardChange>) {
        (self.execution, self.standard_changes)
    }
}
