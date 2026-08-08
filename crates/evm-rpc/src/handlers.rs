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
        EvmServiceError::Simulation(EvmSimulationError::Unsupported(details)) => {
            ValidationError::not_supported(details).into()
        }
        error => {
            let subkind = service_error_subkind(&error);
            error!(subkind, error = ?error, "EVM simulation failed");
            internal_error(subkind, "internal simulation error")
        }
    }
}

fn service_error_subkind(error: &EvmServiceError) -> Option<&'static str> {
    match error {
        EvmServiceError::TaskSetClosed => Some("task_set_closed"),
        EvmServiceError::AttemptTask { .. } => Some("attempt_task_error"),
        EvmServiceError::Simulation(error) => simulation_error_subkind(error),
    }
}

fn simulation_error_subkind(error: &EvmSimulationError) -> Option<&'static str> {
    match error {
        EvmSimulationError::Input(_) | EvmSimulationError::Unsupported(_) => None,
        EvmSimulationError::BlockResolution(_) => Some("block_resolution_error"),
        EvmSimulationError::TransactionCompletion(_) => Some("transaction_completion_error"),
        EvmSimulationError::NotReady(_) => Some("not_ready"),
        EvmSimulationError::BlockContext(_) => Some("block_context_error"),
        EvmSimulationError::StateAccess(_) => Some("state_access_error"),
        EvmSimulationError::Execution(_) => Some("simulation_execution_error"),
        EvmSimulationError::ExecutionTask { .. } => Some("execution_task_error"),
        EvmSimulationError::Changes(_) => Some("analysis_failed"),
        EvmSimulationError::Internal(_) => Some("unexpected"),
        _ => Some("unexpected"),
    }
}
