use alloy::{eips::BlockId, rpc::types::TransactionRequest};
use evm_simulation::{Error, SimulationRequest};
use serde::Deserialize;
use serde_json::Value;

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
    pub(super) fn into_evm(self) -> Result<SimulationRequest, Error> {
        if self.options.is_some() {
            return Err(Error::Unsupported("options are not supported".into()));
        }
        Ok(SimulationRequest {
            block: self.block.unwrap_or_default(),
            transaction: self.transaction,
        })
    }
}
