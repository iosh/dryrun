//! Conflux transaction simulation using the node's executor and RPC state.
mod address;
mod chain;
mod context;
pub mod core_space;
mod endpoint;
mod error;
pub mod espace;
mod execution;
mod primitive;
mod state;
mod tracer;
mod view;

pub use chain::ChainSpec;
pub use error::{Error, StateError};
