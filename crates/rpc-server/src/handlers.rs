use jsonrpsee::core::{RpcResult, async_trait};
use tracing::instrument;

use crate::{
    interface::{EvmSimulateTransactionRequest, EvmSimulateTransactionResponse},
    errors::{ValidationError, not_ready},
    rpc::DryrunRpcServer,
};

pub struct RpcHandler {
    ethereum_rpc_url: String,
}

impl RpcHandler {
    pub fn new(ethereum_rpc_url: String) -> Self {
        Self { ethereum_rpc_url }
    }
}

#[async_trait]
impl DryrunRpcServer for RpcHandler {
    async fn health(&self) -> RpcResult<String> {
        Ok("OK".to_string())
    }

    #[instrument(name = "dryrun_evm_simulateTransaction", skip(self, request))]
    async fn dryrun_evm_simulate_transaction(
        &self,
        request: EvmSimulateTransactionRequest,
    ) -> RpcResult<EvmSimulateTransactionResponse> {
        request
            .validate()
            .map_err(ValidationError::into_error_object)?;

        Err(not_ready(format!(
            "v0 execution path is not wired yet for provider {}",
            self.ethereum_rpc_url
        )))
    }
}
