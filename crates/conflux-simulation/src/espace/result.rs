use simulation_changes::Change;

use super::EspaceExecution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulation {
    pub execution: EspaceExecution,
    pub changes: Vec<Change>,
}

impl EspaceSimulation {
    pub fn new(execution: EspaceExecution, changes: Vec<Change>) -> Self {
        Self { execution, changes }
    }

    pub fn execution(&self) -> &EspaceExecution {
        &self.execution
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    pub fn into_parts(self) -> (EspaceExecution, Vec<Change>) {
        (self.execution, self.changes)
    }
}
