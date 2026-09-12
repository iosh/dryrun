use super::{CoreSpaceBlockContext, CoreSpaceCompleteTransaction, CoreSpaceExecutionOutcome};

/// Availability of verified Core Space state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceChanges {
    Complete(Vec<super::changes::CoreSpaceChange>),
    Unavailable { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulation {
    pub context: CoreSpaceBlockContext,
    pub transaction: CoreSpaceCompleteTransaction,
    outcome: CoreSpaceExecutionOutcome,
    changes: CoreSpaceChanges,
}

impl CoreSpaceSimulation {
    pub(crate) fn new(
        context: CoreSpaceBlockContext,
        transaction: CoreSpaceCompleteTransaction,
        outcome: CoreSpaceExecutionOutcome,
        changes: CoreSpaceChanges,
    ) -> Self {
        Self {
            context,
            transaction,
            outcome,
            changes,
        }
    }

    pub fn outcome(&self) -> &CoreSpaceExecutionOutcome {
        &self.outcome
    }

    pub fn changes(&self) -> &CoreSpaceChanges {
        &self.changes
    }

    pub fn into_parts(
        self,
    ) -> (
        CoreSpaceBlockContext,
        CoreSpaceCompleteTransaction,
        CoreSpaceExecutionOutcome,
        CoreSpaceChanges,
    ) {
        (self.context, self.transaction, self.outcome, self.changes)
    }
}
