use std::sync::Arc;

use crate::{
    ConfluxSimulationError,
    state::{ConfluxSimulationProvider, ConfluxStateAnchor, ConfluxStateSource},
};

mod context;
mod prepared;
mod transaction;

pub use context::{CoreSpaceSimulationContext, EspaceSimulationContext};
pub(crate) use context::{load_core_space_context, load_espace_context};
pub use prepared::{PreparedCoreSpaceSimulation, PreparedEspaceSimulation};
pub(crate) use prepared::{
    PreparedCoreSpaceSimulationState, PreparedEspaceSimulationState, ReadyCoreSpaceSimulation,
    ReadyEspaceSimulation,
};
pub(crate) use transaction::{complete_core_space_transaction, complete_espace_transaction};

pub(crate) async fn prepare_state_source(
    provider: Arc<ConfluxSimulationProvider>,
    state_anchor: ConfluxStateAnchor,
) -> Result<ConfluxStateSource, ConfluxSimulationError> {
    ConfluxStateSource::prepare(state_anchor, provider)
        .await
        .map_err(|error| ConfluxSimulationError::StateAccess {
            message: error.to_string(),
        })
}
