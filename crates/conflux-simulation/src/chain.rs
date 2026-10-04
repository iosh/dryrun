mod mainnet;

use cfx_executor::{
    machine::{Machine, VmFactory},
    spec::CommonParams,
};
use conflux_provider::Network;

/// Conflux execution parameters and address network.
#[derive(Debug, Clone)]
pub struct ChainSpec {
    pub(crate) params: CommonParams,
    pub(crate) network: Network,
}

impl ChainSpec {
    pub fn mainnet() -> Self {
        Self {
            params: mainnet::params(),
            network: Network::Main,
        }
    }

    pub(crate) fn machine(&self) -> Machine {
        Machine::new_with_builtin(self.params.clone(), VmFactory::default())
    }
}
