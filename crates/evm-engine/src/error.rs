use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmEngineError {
    #[error("{0}")]
    NotSupported(String),

    #[error("{details}")]
    Internal {
        subkind: &'static str,
        details: String,
    },
}

impl EvmEngineError {
    pub fn not_supported(details: impl Into<String>) -> Self {
        Self::NotSupported(details.into())
    }

    pub fn not_ready(details: impl Into<String>) -> Self {
        Self::Internal {
            subkind: "not_ready",
            details: details.into(),
        }
    }

    pub fn block_not_found(details: impl Into<String>) -> Self {
        Self::Internal {
            subkind: "block_not_found",
            details: details.into(),
        }
    }

    pub fn rpc_error(details: impl Into<String>) -> Self {
        Self::Internal {
            subkind: "rpc_error",
            details: details.into(),
        }
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::Internal {
            subkind: "unexpected",
            details: details.into(),
        }
    }
}
