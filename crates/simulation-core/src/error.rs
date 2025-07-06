use alloy::transports::{RpcError, TransportErrorKind};
use revm::{
    context::{result::EVMError, tx::TxEnvBuildError},
    database::DBTransportError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimulationError {
    #[error("RPC Error: {0}")]
    ProviderError(#[from] RpcError<TransportErrorKind>),

    #[error("invalid tx: {0:?}")]
    InvalidTransaction(TxEnvBuildError),

    #[error("db error: {0}")]
    DatabaseError(#[from] DBTransportError),

    #[error("execution error: {0}")]
    ExecutionError(#[from] EVMError<DBTransportError>),

    #[error("block number not found")]
    BlockNumberNotFound,
}

pub type SimulationResult<T> = Result<T, SimulationError>;
