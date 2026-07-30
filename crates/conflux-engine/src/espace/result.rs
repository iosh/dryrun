use contract_standards::StandardChange;

use super::EspaceExecution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulation {
    pub execution: EspaceExecution,
    pub changes: Vec<StandardChange>,
}

impl EspaceSimulation {
    pub fn new(execution: EspaceExecution, changes: Vec<StandardChange>) -> Self {
        Self { execution, changes }
    }

    pub fn execution(&self) -> &EspaceExecution {
        &self.execution
    }

    pub fn changes(&self) -> &[StandardChange] {
        &self.changes
    }

    pub fn into_parts(self) -> (EspaceExecution, Vec<StandardChange>) {
        (self.execution, self.changes)
    }
}
