use tokio::runtime::Handle;

use crate::{ConfluxSimulationError, PreparedCoreSpaceSimulation};

use super::{CoreSpaceSimulation, simulation};

#[derive(Clone)]
pub struct CoreSpaceSimulator {
    runtime_handle: Handle,
}

impl CoreSpaceSimulator {
    pub fn new(runtime_handle: Handle) -> Self {
        Self { runtime_handle }
    }

    pub fn simulate(
        &self,
        prepared_simulation: PreparedCoreSpaceSimulation,
    ) -> Result<CoreSpaceSimulation, ConfluxSimulationError> {
        simulation::simulate(prepared_simulation, &self.runtime_handle)
    }
}
