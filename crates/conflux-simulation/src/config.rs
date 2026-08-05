use cfx_addr::Network;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxChainConfig {
    pub core_space_chain_id: u32,
    pub evm_chain_id: u32,
    pub core_space_address_network: Network,
}

impl ConfluxChainConfig {
    pub fn mainnet() -> Self {
        Self {
            core_space_chain_id: 1029,
            evm_chain_id: 1030,
            core_space_address_network: Network::Main,
        }
    }
}
