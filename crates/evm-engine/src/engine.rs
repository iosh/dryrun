use crate::{EvmEngineError, EvmExecutionInput, EvmSimulation, execution::simulate_execution};

#[derive(Debug, Clone)]
pub struct EvmEngine {
    rpc_url: String,
}

impl EvmEngine {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub async fn simulate(
        &self,
        input: EvmExecutionInput,
    ) -> Result<EvmSimulation, EvmEngineError> {
        simulate_execution(&self.rpc_url, input).await
    }
}
