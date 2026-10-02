use alloy::{eips::BlockId, rpc::types::TransactionRequest};
use jsonrpsee::types::ErrorObjectOwned;
use serde::Deserialize;
use serde_json::Value;
use simulation_core::ErrorCode;

/// Parameters of the block-based simulation methods.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockRequest {
    transaction: TransactionRequest,
    #[serde(default)]
    block: Option<BlockId>,
    /// Reserved for state and block overrides.
    #[serde(default)]
    options: Option<Value>,
}

impl BlockRequest {
    pub(super) fn into_parts(self) -> Result<(BlockId, TransactionRequest), ErrorObjectOwned> {
        if self.options.is_some() {
            return Err(super::error::rpc_error_object(
                ErrorCode::Unsupported,
                "options are not supported".into(),
            ));
        }
        Ok((self.block.unwrap_or_default(), self.transaction))
    }
}
