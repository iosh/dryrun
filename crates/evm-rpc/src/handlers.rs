use evm_simulation::{EvmSimulationError, EvmSimulationRequest, EvmTransactionSimulator};
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::types::ErrorObjectOwned;
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
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
    simulator: EvmTransactionSimulator,
    simulation_tasks: SimulationTaskSet,
}

impl RpcHandler {
    pub fn new(simulator: EvmTransactionSimulator, simulation_tasks: SimulationTaskSet) -> Self {
        Self {
            simulator,
            simulation_tasks,
        }
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
        let simulator = self.simulator.clone();
        let output = self
            .simulation_tasks
            .run(move || async move { simulator.simulate(input).await })
            .await
            .map_err(map_task_error)?
            .map_err(map_simulation_error)?;

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

fn map_simulation_error(error: EvmSimulationError) -> ErrorObjectOwned {
    match error {
        EvmSimulationError::Input(error) => {
            ValidationError::invalid_params(error.to_string()).into()
        }
        EvmSimulationError::NotReady(error) => {
            ValidationError::not_supported(error.to_string()).into()
        }
        error => {
            error!(error = ?error, "EVM simulation failed");
            internal_error()
        }
    }
}

fn map_task_error(error: SimulationTaskError) -> ErrorObjectOwned {
    error!(error = ?error, "EVM simulation task failed");
    internal_error()
}
