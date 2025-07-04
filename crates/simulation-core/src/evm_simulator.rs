use types::{EvmSimulateInput, EvmSimulateOutput};

pub struct EvmSimulator {}

impl EvmSimulator {
    pub fn new() -> Self {
        EvmSimulator {}
    }

    pub async fn simulate(&self, input: EvmSimulateInput) -> Result<EvmSimulateOutput, String> {
        // TODO

        unimplemented!()
    }
}
