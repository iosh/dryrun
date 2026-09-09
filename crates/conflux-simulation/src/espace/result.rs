use super::{EspaceBlockContext, EspaceChanges, EspaceCompleteTransaction, EspaceExecutionOutcome};

#[derive(Debug)]
pub struct EspaceSimulation {
    pub context: EspaceBlockContext,
    pub transaction: EspaceCompleteTransaction,
    pub execution: EspaceExecutionOutcome,
    pub changes: EspaceChanges,
}
