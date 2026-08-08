use std::sync::Arc;

use evm_service::{EvmServiceError, EvmSimulationService};
use evm_simulation::{EvmSimulationError, EvmSimulationRequest};
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::types::ErrorObjectOwned;
use tracing::{error, instrument};

use crate::{
    errors::{ValidationError, internal_error},
    interface::{
        BlockRef, EvmSimulateTransactionRequest, EvmSimulateTransactionResponse,
        SimulateTransactionOptions, Transaction,
    },
    rpc::DryrunRpcServer,
};

#[derive(Clone)]
pub struct RpcHandler {
    simulation_service: Arc<EvmSimulationService>,
}

impl RpcHandler {
    pub fn new(simulation_service: Arc<EvmSimulationService>) -> Self {
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
        let input: EvmSimulationRequest = request.try_into()?;
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

fn map_service_error(error: EvmServiceError) -> ErrorObjectOwned {
    match error {
        EvmServiceError::Simulation(EvmSimulationError::Input(error)) => {
            ValidationError::invalid_params(error.to_string()).into()
        }
        EvmServiceError::Simulation(EvmSimulationError::NotReady(error)) => {
            ValidationError::not_supported(error.to_string()).into()
        }
        error => {
            error!(error = ?error, "EVM simulation failed");
            internal_error()
        }
    }
}
