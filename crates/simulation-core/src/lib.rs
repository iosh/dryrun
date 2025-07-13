mod error;
mod evm_simulator;
mod inspector;
use std::sync::Arc;

use crate::error::SimulationResult;
use configs::EvmConfig;
use evm_simulator::EvmSimulator;
use types::EvmSimulateOutput;

pub struct SimulationService {
    evm_simulator: Arc<EvmSimulator>,
}

impl SimulationService {
    pub fn new(config: EvmConfig) -> Self {
        let rpc_url = config.rpc_url;
        let evm_simulator = Arc::new(EvmSimulator::new(&rpc_url));
        Self { evm_simulator }
    }

    pub async fn run_evm_simulation(
        &self,
        input: types::EvmSimulateInput,
    ) -> SimulationResult<EvmSimulateOutput> {
        self.evm_simulator.simulate(input).await
    }
}
