use std::sync::Arc;

use alloy::rpc::types::{BlockId, BlockOverrides, TransactionRequest, state::StateOverride};
use jsonrpsee::{
    core::{RpcResult, async_trait},
    types::ErrorObject,
};
use simulation_core::SimulationService;
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

    async fn dryrun_evm_simulate_transaction(
        &self,
        transaction: TransactionRequest,
        block_id: BlockId,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<BlockOverrides>,
    ) -> RpcResult<EvmSimulateOutput> {
        let input = EvmSimulateInput::new(transaction, block_id, state_overrides, block_overrides);

        let result = self.service.run_evm_simulation(input).await;

        match result {
            Ok(output) => Ok(output),
            Err(err) => {
                tracing::error!("Simulation failed: {}", err);
                Err(ErrorObject::owned(
                    -32000,
                    "Simulation failed",
                    Some(err.to_string()),
                ))
            }
        }
    }
}
