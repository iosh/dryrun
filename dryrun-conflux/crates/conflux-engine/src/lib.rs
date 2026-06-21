pub mod config;
mod error;
pub mod execution;
mod simulation;
pub mod state;
mod transaction;

use std::sync::Arc;

pub use error::ConfluxEngineError;
pub use simulation::{
    EspaceExecution, EspaceExecutionFailure, EspaceExecutionStatus, EspaceSimulation,
    SimulatedBlock,
};
pub use transaction::{
    AccessListItem, EspaceBlockRef, EspaceTransaction, EspaceTransactionType,
    SimulateEspaceTransactionInput,
};

use crate::{
    config::ConfluxConfig,
    state::{HttpConfluxStateProvider, RemoteStateProvider},
};

pub struct ConfluxEngine {
    config: ConfluxConfig,
    provider: Arc<dyn RemoteStateProvider>,
}

impl ConfluxEngine {
    pub fn new(config: ConfluxConfig) -> Result<Self, ConfluxEngineError> {
        let provider = Arc::new(HttpConfluxStateProvider::new(config.clone())?);
        Ok(Self { config, provider })
    }

    pub fn with_provider(
        config: ConfluxConfig,
        provider: Arc<dyn RemoteStateProvider>,
    ) -> Self {
        Self { config, provider }
    }

    pub fn simulate_espace_transaction(
        &self,
        input: SimulateEspaceTransactionInput,
    ) -> Result<EspaceSimulation, ConfluxEngineError> {
        let _ = (&self.config, &self.provider, input);

        Err(ConfluxEngineError::Unexpected {
            message: "eSpace execution facade is not implemented yet".to_string(),
        })
    }
}
