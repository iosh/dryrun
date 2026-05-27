mod error;
pub mod state;
mod types;

pub use error::ConfluxEngineError;
pub use types::{ConfluxExecutionInput, ConfluxSimulation};

#[derive(Debug, Clone)]
pub struct ConfluxEngine {
    rpc_url: String,
}

impl ConfluxEngine {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }
}
