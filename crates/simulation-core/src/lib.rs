mod evm_simulator;
use std::sync::Arc;

use evm_simulator::EvmSimulator;
pub struct SimulationService {
    evm_simulator: Arc<EvmSimulator>,
}

impl SimulationService {
    pub fn new() -> Self {
        let evm_simulator = Arc::new(EvmSimulator::new());
        Self { evm_simulator }
    }

    pub async fn run_evm_simulation(
        &self,
        input: types::EvmSimulateInput,
    ) -> Result<types::EvmSimulateOutput, String> {
        self.evm_simulator.simulate(input).await
    }
}
