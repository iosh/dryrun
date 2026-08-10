use super::{EspaceBlockContext, EspaceChange, EspaceCompleteTransaction, EspaceExecutionOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulation {
    pub context: EspaceBlockContext,
    pub transaction: EspaceCompleteTransaction,
    pub execution: EspaceExecutionOutcome,
    pub changes: Vec<EspaceChange>,
}
