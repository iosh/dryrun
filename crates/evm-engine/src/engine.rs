use alloy::providers::DynProvider;
use alloy_chains::Chain;
use tokio::runtime::Handle;

use crate::{EvmEngineError, EvmExecutionInput, EvmSimulation, execution::simulate_execution};

#[derive(Debug, Clone)]
pub struct EvmEngine {
    provider: DynProvider,
    runtime_handle: Handle,
    chain_id: u64,
}

impl EvmEngine {
    pub fn new(provider: DynProvider, runtime_handle: Handle) -> Self {
        Self {
            provider,
            runtime_handle,
            chain_id: Chain::mainnet().id(),
        }
    }

    pub fn simulate(&self, input: EvmExecutionInput) -> Result<EvmSimulation, EvmEngineError> {
        simulate_execution(&self.provider, &self.runtime_handle, self.chain_id, input)
    }
}
