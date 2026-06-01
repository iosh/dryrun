use cfx_types::{Address, H256, U256};
use thiserror::Error;

/// Remote state access
pub(crate) trait RemoteStateProvider: Send + Sync {
    fn get_espace_storage_at(
        &self,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;
}

/// HTTP-backed eSpace state reader.
#[derive(Debug, Clone)]
pub(crate) struct HttpEspaceProvider {
    rpc_url: String,
}

impl HttpEspaceProvider {
    pub(crate) fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub(crate) fn rpc_url(&self) -> &str {
        &self.rpc_url
    }
}

impl RemoteStateProvider for HttpEspaceProvider {
    fn get_espace_storage_at(
        &self,
        _address: Address,
        _slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        Err(RemoteStateProviderError::Unimplemented {
            method: "eth_getStorageAt",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum RemoteStateProviderError {
    #[error("remote state provider method is not implemented: {method}")]
    Unimplemented { method: &'static str },

    #[error("remote state request failed: {message}")]
    RequestFailed { message: String },
}
