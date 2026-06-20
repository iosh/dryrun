use cfx_addr::Network;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxConfig {
    pub chain: ConfluxChainConfig,
    pub rpc: ConfluxRpcConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxChainConfig {
    pub native_chain_id: u32,
    pub evm_chain_id: u32,
    pub native_address_network: Network,
}

impl ConfluxChainConfig {
    pub fn mainnet() -> Self {
        Self {
            native_chain_id: 1029,
            evm_chain_id: 1030,
            native_address_network: Network::Main,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxRpcConfig {
    pub evm_url: String,
    pub native_url: String,
}
