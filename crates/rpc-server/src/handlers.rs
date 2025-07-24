use std::sync::Arc;

use alloy::rpc::types::{BlockId, BlockOverrides, TransactionRequest, state::StateOverride};
use jsonrpsee::{
    core::{RpcResult, async_trait},
    tokio::time::Instant,
    types::ErrorObject,
};
use metrics::{counter, histogram};
use simulation_core::SimulationService;
use tracing::instrument;
use types::{EvmSimulateInput, EvmSimulateOutput};

use crate::rpc::SimulationRpcServer;

pub struct RpcHandler {
    service: Arc<SimulationService>,
}

impl RpcHandler {
    pub fn new(service: Arc<SimulationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl SimulationRpcServer for RpcHandler {
    async fn health(&self) -> RpcResult<String> {
        Ok("OK".to_string())
    }

    #[instrument(
        name = "dryrun_evm_simulate_transaction",
        skip(self, transaction, state_overrides, block_overrides),
        fields(
            tx_from = ?transaction.from,
            tx_to = ?transaction.to,
            tx_value = ?transaction.value,
            block_id = ?block_id,
        )
    )]
    async fn dryrun_evm_simulate_transaction(
        &self,
        transaction: TransactionRequest,
        block_id: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<BlockOverrides>,
    ) -> RpcResult<EvmSimulateOutput> {
        let start = Instant::now();
        let rpc_method = "dryrun_evm_simulate_transaction";

        counter!("dryrun_rpc_requests_total", "method" => rpc_method).increment(1);
        let input = EvmSimulateInput::new(transaction, block_id, state_overrides, block_overrides);

        let result = self.service.run_evm_simulation(input).await;

        let duration = start.elapsed().as_secs_f64();

        histogram!("dryrun_rpc_request_duration_seconds", "method" => rpc_method).record(duration);
        match result {
            Ok(output) => Ok(output),
            Err(err) => {
                counter!("dryrun_rpc_requests_failed_total", "method" => rpc_method).increment(1);
                tracing::error!(error = ?err, "Simulation failed");
                Err(ErrorObject::owned(
                    -32000,
                    "Simulation failed",
                    Some(err.to_string()),
                ))
            }
        }
    }
}
