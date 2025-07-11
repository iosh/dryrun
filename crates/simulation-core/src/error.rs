use alloy::transports::{RpcError, TransportErrorKind};
use revm::{
    bytecode::BytecodeDecodeError,
    context::{result::EVMError, tx::TxEnvBuildError},
    database::DBTransportError, primitives::Address,
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

    #[error("bytecode decode error: {0}")]
    BytecodeDecodeError(#[from] BytecodeDecodeError),

    #[error("state override error: {0}")]
    BothStateAndStateDiff(Address)
}

pub type SimulationResult<T> = Result<T, SimulationError>;
