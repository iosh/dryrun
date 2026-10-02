use alloy_transport::TransportError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Core Space provider request {method} failed")]
    Rpc {
        method: &'static str,
        #[source]
        source: TransportError,
    },
}
