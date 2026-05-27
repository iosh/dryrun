#[derive(Debug, thiserror::Error)]
pub enum ConfluxEngineError {
    #[error("conflux state mapping failed: {0}")]
    StateMapping(String),
}
