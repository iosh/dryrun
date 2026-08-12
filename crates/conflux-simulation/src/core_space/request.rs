use super::{CoreSpaceBlockSelector, CoreSpaceTransactionInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulationRequest {
    pub block: CoreSpaceBlockSelector,
    pub transaction: CoreSpaceTransactionInput,
}
