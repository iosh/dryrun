use std::sync::Arc;

use evm_service::{SimulationService, SimulationServiceError};
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::types::ErrorObjectOwned;
use tracing::instrument;

use crate::{
    errors::{internal_error, not_supported},
    interface::{
        BlockRef, EvmSimulateTransactionRequest, EvmSimulateTransactionResponse,
        SimulateTransactionOptions, Transaction,
    },
    rpc::DryrunRpcServer,
};

#[derive(Clone)]
pub struct RpcHandler {
    simulation_service: Arc<SimulationService>,
}

impl RpcHandler {
    pub fn new(simulation_service: Arc<SimulationService>) -> Self {
        Self { simulation_service }
    }

    #[instrument(
        name = "dryrun_evm_simulateTransaction",
        skip(self, transaction, block, options)
    )]
    async fn handle_simulate_transaction(
        &self,
        transaction: Transaction,
        block: Option<BlockRef>,
        options: Option<SimulateTransactionOptions>,
    ) -> RpcResult<EvmSimulateTransactionResponse> {
        let request = EvmSimulateTransactionRequest {
            transaction,
            block,
            options,
        };
        let input: evm_service::SimulateEvmTransactionInput =
            request.try_into().map_err(ErrorObjectOwned::from)?;
        let output = self
            .simulation_service
            .simulate_evm_transaction(input)
            .await
            .map_err(map_service_error)?;

        Ok(output.into())
    }
}

#[async_trait]
impl DryrunRpcServer for RpcHandler {
    async fn health(&self) -> RpcResult<String> {
        Ok("OK".to_string())
    }

    async fn dryrun_evm_simulate_transaction(
        &self,
        transaction: Transaction,
        block: Option<BlockRef>,
        options: Option<SimulateTransactionOptions>,
    ) -> RpcResult<EvmSimulateTransactionResponse> {
        self.handle_simulate_transaction(transaction, block, options)
            .await
    }
}

fn map_service_error(error: SimulationServiceError) -> ErrorObjectOwned {
    let details = error.details().to_owned();

    if error.is_not_supported() {
        not_supported(details)
    } else {
        internal_error(error.kind_code(), details)
    }
}

#[cfg(test)]
mod tests {
    use evm_service::SimulationServiceError;
    use serde_json::to_value;

    use super::map_service_error;

    #[test]
    fn admission_timeout_keeps_the_internal_rpc_error_shape() {
        let error = map_service_error(SimulationServiceError::AdmissionTimedOut);
        let value = to_value(error).expect("RPC error should serialize");

        assert_eq!(value["code"], -32603);
        assert_eq!(value["message"], "Internal error");
        assert_eq!(value["data"]["subkind"], "admission_timed_out");
        assert_eq!(
            value["data"]["details"],
            "timed out waiting for simulation capacity"
        );
    }
}
