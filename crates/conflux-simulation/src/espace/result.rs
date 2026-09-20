use super::{EspaceBlockContext, EspaceChanges, EspaceExecutionOutcome, EspaceTypedTransaction};

#[derive(Debug)]
pub struct EspaceSimulation {
    pub context: EspaceBlockContext,
    pub transaction: EspaceTypedTransaction,
    pub execution: EspaceExecutionOutcome,
    pub changes: EspaceChanges,
}
