mod evm_simulator;
use std::sync::Arc;

use configs::EvmConfig;
use evm_simulator::EvmSimulator;
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
    ) -> Result<types::EvmSimulateOutput, String> {
        self.evm_simulator.simulate(input).await
    }
}
