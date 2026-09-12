use super::{
    CoreSpaceBlockContext, CoreSpaceChangeDerivationError, CoreSpaceChangeSet,
    CoreSpaceCompleteTransaction, CoreSpaceExecutionOutcome,
};

/// Availability of verified Core Space state changes.
#[derive(Debug)]
pub enum CoreSpaceChanges {
    Complete(CoreSpaceChangeSet),
    Unavailable(CoreSpaceChangeDerivationError),
}

impl From<Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError>> for CoreSpaceChanges {
    fn from(result: Result<CoreSpaceChangeSet, CoreSpaceChangeDerivationError>) -> Self {
        match result {
            Ok(changes) => Self::Complete(changes),
            Err(error) => Self::Unavailable(error),
        }
    }
}

#[derive(Debug)]
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
