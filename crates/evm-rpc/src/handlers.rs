use evm_simulation::{EvmSimulationRequest, EvmTransactionSimulator};
use jsonrpsee::core::{RpcResult, async_trait};
use simulation_tasks::SimulationTaskSet;
use tracing::instrument;

use crate::{
    errors::rpc_error,
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
            .map_err(rpc_error)?
            .map_err(rpc_error)?;

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
