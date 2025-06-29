use jsonrpsee::core::{RpcResult, async_trait};

use crate::rpc::SimulationRpcServer;

pub struct RpcHandler {}

impl Default for RpcHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcHandler {
    pub fn new() -> Self {
        RpcHandler {}
    }
}

#[async_trait]
impl SimulationRpcServer for RpcHandler {
    async fn health(&self) -> RpcResult<String> {
        Ok("OK".to_string())
    }
}
