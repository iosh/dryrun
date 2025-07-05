use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder};
use types::{EvmSimulateInput, EvmSimulateOutput};

pub struct EvmSimulator {
    provider: Arc<dyn Provider>,
}

impl EvmSimulator {
    pub fn new(rpc_url: &str) -> Self {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse().unwrap());

        EvmSimulator {
            provider: Arc::new(provider),
        }
    }

    pub async fn simulate(&self, input: EvmSimulateInput) -> Result<EvmSimulateOutput, String> {
        unimplemented!()
    }
}
