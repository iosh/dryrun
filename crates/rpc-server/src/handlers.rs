use std::sync::Arc;

use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::types::ErrorObjectOwned;
use simulation_service::{SimulationService, SimulationServiceError};
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
        let request = EvmSimulateTransactionRequest::new(transaction, block, options);
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
    match error {
        SimulationServiceError::NotSupported(details) => not_supported(details),
        SimulationServiceError::Internal { subkind, details } => {
            internal_error(Some(subkind), details)
        }
    }
}
