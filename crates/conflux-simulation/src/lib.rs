mod backend;
#[cfg(feature = "serde")]
mod codec;

mod chain_spec;
mod context;
pub mod core_space;
mod error;
pub mod espace;
mod execution;
mod primitive;
mod state;

pub use backend::ConfluxSimulationBackend;
pub use context::ConfluxBlockContextError;
pub use error::{
    ConfluxCoreStatusIdentityField, ConfluxEndpointIdentity, ConfluxInitializationError,
    ConfluxStateAnchorError,
};
pub use state::ConfluxRpcError;

pub use simulation_core::analysis::{
    AnalysisReport, AnalysisScope, Analyzer, AnalyzerDescriptor, AnalyzerLayer, ChainScope,
    Deployment, ExecutionFact, ExecutionSpace, FactKind, RegistryError, SupportEvidence,
};
pub use simulation_core::contract_analysis::{
    ReviewedStandardImplementation, StandardAnalyzer, Weth9Analyzer,
};

pub use simulation_core::observation::LogFilter;
