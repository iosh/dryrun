use std::sync::Arc;

use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::types::ErrorObjectOwned;
use simulation_service::{SimulationService, SimulationServiceError};
use tracing::instrument;

use crate::{
    errors::{internal_error, not_ready},
    interface::{EvmSimulateTransactionRequest, EvmSimulateTransactionResponse},
    rpc::DryrunRpcServer,
};

pub struct RpcHandler {
    simulation_service: Arc<SimulationService>,
}

impl RpcHandler {
    pub fn new(simulation_service: Arc<SimulationService>) -> Self {
        Self { simulation_service }
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
        let input: simulation_service::SimulateEvmTransactionInput =
            request.try_into().map_err(ErrorObjectOwned::from)?;
        let output = self
            .simulation_service
            .simulate_evm_transaction(input)
            .await
            .map_err(map_service_error)?;

        Ok(output.into())
    }
}

fn map_service_error(error: SimulationServiceError) -> ErrorObjectOwned {
    match error {
        SimulationServiceError::NotReady(details) => not_ready(details),
        SimulationServiceError::Internal(details) => internal_error(details),
    }
}
